//! Credential-free model identity newtypes.
//!
//! [`CanonicalModelId`] is the authoritative **selection** identifier used by
//! pickers, config, defaults, persistence, ACP, and tool calls. It may be an
//! existing curated id, a discovered compatibility id, or an explicit user
//! `[model.<id>]` key. It is not required to contain a colon.
//!
//! [`UpstreamModelId`] is the exact provider-wire model string. It may contain
//! `/`, `:`, `.`, `-`, `@`, `%`, `?`, and `#`, and is never silently normalized.
//!
//! For newly generated/discovered provider models the canonical form is
//! `<provider-instance-id>:<upstream-model-id>` with the **first** colon as
//! the separator; the remainder is the verbatim upstream id.
//!
//! These types serialize as plain strings (`#[serde(transparent)]`) so existing
//! JSON/TOML fixtures stay byte-compatible. They reject control characters,
//! credential-shaped material, and true URL/userinfo credentials. They never
//! store secrets.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

/// Maximum UTF-8 length of a canonical selection id or upstream wire id.
pub const MAX_MODEL_ID_LEN: usize = 256;

/// Built-in provider instance prefixes that stay bound to built-in accounts.
pub const BUILTIN_PROVIDER_PREFIXES: &[&str] =
    &["openai", "openrouter", "xai", "anthropic", "grok"];

/// Authoritative catalog / picker / persistence selection identifier.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(transparent)]
pub struct CanonicalModelId(String);

/// Exact provider-wire model string. Never normalized.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(transparent)]
pub struct UpstreamModelId(String);

/// Optional secret-free route sidecar persisted next to a canonical selection.
///
/// Additive: absence means a pre-upgrade reference. Never contains credentials,
/// display names, custom URLs, or organization/project identifiers.
///
/// `schema_version` 0 = pre-upgrade / partial fields (legacy unique-alias).
/// `schema_version` 1 = exact route: every stored pin must match live.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct ModelRouteProvenance {
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub schema_version: u8,
    pub provider_instance_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_surface: Option<String>,
    pub upstream_model: String,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub registry_generation: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_model: Option<String>,
    /// Secret-free pair token binding this companion to the matching summary write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair_id: Option<String>,
}

/// Exact-route sidecar schema. Present incarnation implies this version.
pub const PROVENANCE_SCHEMA_EXACT: u8 = 1;
/// Legacy sidecar without incarnation pins. Present fields still fail closed.
pub const PROVENANCE_SCHEMA_LEGACY: u8 = 0;

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

fn is_zero_u8(value: &u8) -> bool {
    *value == 0
}

impl<'de> Deserialize<'de> for CanonicalModelId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for UpstreamModelId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for ModelRouteProvenance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            schema_version: u8,
            provider_instance_id: String,
            #[serde(default)]
            incarnation: Option<String>,
            #[serde(default)]
            provider_kind: Option<String>,
            #[serde(default)]
            api_surface: Option<String>,
            upstream_model: String,
            #[serde(default)]
            registry_generation: u64,
            #[serde(default)]
            canonical_model: Option<String>,
            #[serde(default)]
            pair_id: Option<String>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let upstream = UpstreamModelId::new(raw.upstream_model).map_err(de::Error::custom)?;
        if raw.schema_version > PROVENANCE_SCHEMA_EXACT {
            return Err(de::Error::custom(
                "unsupported model route provenance version",
            ));
        }
        if raw.schema_version == PROVENANCE_SCHEMA_EXACT && raw.registry_generation == 0 {
            return Err(de::Error::custom(
                "exact route provenance requires a nonzero registry generation",
            ));
        }
        let mut provenance = ModelRouteProvenance::new(
            raw.provider_instance_id,
            raw.incarnation,
            raw.provider_kind,
            raw.api_surface,
            &upstream,
            raw.registry_generation,
        )
        .map_err(de::Error::custom)?;
        if provenance.has_incarnation() {
            provenance.schema_version = PROVENANCE_SCHEMA_EXACT;
        } else {
            provenance.schema_version = raw.schema_version.min(PROVENANCE_SCHEMA_EXACT);
        }
        if provenance.schema_version == PROVENANCE_SCHEMA_EXACT
            && provenance.registry_generation == 0
        {
            return Err(de::Error::custom(
                "exact route provenance requires a nonzero registry generation",
            ));
        }
        if let Some(canonical) = raw.canonical_model {
            let id = CanonicalModelId::new(canonical).map_err(de::Error::custom)?;
            provenance.canonical_model = Some(id.into_string());
        }
        if let Some(pair_id) = raw.pair_id {
            provenance = provenance
                .with_pair_id(pair_id)
                .map_err(de::Error::custom)?;
        }
        Ok(provenance)
    }
}

/// Why a model-id string was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelIdError {
    Empty,
    TooLong { len: usize },
    ControlChar,
    UnsafeIdentity,
    InvalidGeneration,
}

impl fmt::Display for ModelIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "model id is empty"),
            Self::TooLong { len } => {
                write!(
                    f,
                    "model id length {len} exceeds maximum {MAX_MODEL_ID_LEN}"
                )
            }
            Self::ControlChar => write!(f, "model id contains a control character"),
            Self::UnsafeIdentity => {
                write!(f, "model id looks like a secret, URL, or unsafe identity")
            }
            Self::InvalidGeneration => {
                write!(
                    f,
                    "exact route provenance requires a nonzero registry generation"
                )
            }
        }
    }
}

impl std::error::Error for ModelIdError {}

impl CanonicalModelId {
    pub fn new(raw: impl AsRef<str>) -> Result<Self, ModelIdError> {
        Ok(Self(validate_model_id_str(raw.as_ref())?))
    }

    /// Discovered form: `{provider_instance_id}:{upstream}` (first colon only).
    pub fn discovered(
        provider_instance_id: &str,
        upstream: &UpstreamModelId,
    ) -> Result<Self, ModelIdError> {
        let prefix = validate_safe_label(provider_instance_id, 64)?;
        let combined = format!("{prefix}:{}", upstream.as_str());
        Self::new(combined)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn split_provider_prefix(&self) -> Option<(&str, &str)> {
        split_first_colon(&self.0)
    }

    pub fn is_builtin_namespaced(&self) -> bool {
        match self.split_provider_prefix() {
            Some((prefix, _)) => is_builtin_provider_prefix(prefix),
            None => false,
        }
    }

    pub fn is_reserved_compatibility_selection(&self) -> bool {
        match self.split_provider_prefix() {
            None => true,
            Some((prefix, remainder)) => {
                !remainder.is_empty() && is_builtin_provider_prefix(prefix)
            }
        }
    }
}

impl UpstreamModelId {
    pub fn new(raw: impl AsRef<str>) -> Result<Self, ModelIdError> {
        Ok(Self(validate_model_id_str(raw.as_ref())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl ModelRouteProvenance {
    pub fn new(
        provider_instance_id: impl AsRef<str>,
        incarnation: Option<impl AsRef<str>>,
        provider_kind: Option<impl AsRef<str>>,
        api_surface: Option<impl AsRef<str>>,
        upstream: &UpstreamModelId,
        registry_generation: u64,
    ) -> Result<Self, ModelIdError> {
        let provider_instance_id = validate_safe_label(provider_instance_id.as_ref(), 64)?;
        let incarnation = match incarnation {
            Some(raw) => {
                let s = validate_safe_label(raw.as_ref(), 64)?;
                Some(s)
            }
            None => None,
        };
        let provider_kind = match provider_kind {
            Some(raw) => Some(validate_safe_label(raw.as_ref(), 32)?),
            None => None,
        };
        let api_surface = match api_surface {
            Some(raw) => Some(validate_safe_label(raw.as_ref(), 32)?),
            None => None,
        };
        let schema_version = if incarnation.is_some() {
            if registry_generation == 0 {
                return Err(ModelIdError::InvalidGeneration);
            }
            PROVENANCE_SCHEMA_EXACT
        } else {
            PROVENANCE_SCHEMA_LEGACY
        };
        Ok(Self {
            schema_version,
            provider_instance_id,
            incarnation,
            provider_kind,
            api_surface,
            upstream_model: upstream.as_str().to_owned(),
            registry_generation,
            canonical_model: None,
            pair_id: None,
        })
    }

    pub fn with_canonical_model(mut self, canonical: &CanonicalModelId) -> Self {
        self.canonical_model = Some(canonical.as_str().to_owned());
        self
    }

    pub fn with_pair_id(mut self, pair_id: impl AsRef<str>) -> Result<Self, ModelIdError> {
        self.pair_id = Some(validate_safe_label(pair_id.as_ref(), 64)?);
        Ok(self)
    }

    pub fn has_incarnation(&self) -> bool {
        self.incarnation.is_some()
    }

    pub fn requires_exact_route(&self) -> bool {
        self.schema_version == PROVENANCE_SCHEMA_EXACT || self.has_incarnation()
    }

    pub fn matches_live(
        &self,
        live_instance_id: Option<&str>,
        live_incarnation: Option<&str>,
        live_kind: Option<&str>,
        live_surface: Option<&str>,
        live_upstream: Option<&str>,
        live_registry_generation: Option<u64>,
    ) -> bool {
        if live_instance_id != Some(self.provider_instance_id.as_str()) {
            return false;
        }
        if live_upstream != Some(self.upstream_model.as_str()) {
            return false;
        }
        if self.provider_kind.is_some() && live_kind != self.provider_kind.as_deref() {
            return false;
        }
        if self.api_surface.is_some() && live_surface != self.api_surface.as_deref() {
            return false;
        }
        if self.has_incarnation() {
            if live_incarnation != self.incarnation.as_deref() {
                return false;
            }
            // Exact generation is not a wildcard: must compare unconditionally.
            if live_registry_generation != Some(self.registry_generation) {
                return false;
            }
        } else if self.registry_generation != 0
            && live_registry_generation.is_some_and(|g| g != self.registry_generation)
        {
            return false;
        }
        true
    }
}

impl fmt::Display for CanonicalModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for UpstreamModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for CanonicalModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CanonicalModelId").field(&self.0).finish()
    }
}

impl fmt::Debug for UpstreamModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("UpstreamModelId").field(&self.0).finish()
    }
}

impl fmt::Debug for ModelRouteProvenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelRouteProvenance")
            .field("schema_version", &self.schema_version)
            .field("provider_instance_id", &self.provider_instance_id)
            .field("incarnation", &self.incarnation)
            .field("provider_kind", &self.provider_kind)
            .field("api_surface", &self.api_surface)
            .field("upstream_model", &self.upstream_model)
            .field("registry_generation", &self.registry_generation)
            .field("canonical_model", &self.canonical_model)
            .field("pair_id", &self.pair_id)
            .finish()
    }
}

impl FromStr for CanonicalModelId {
    type Err = ModelIdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl FromStr for UpstreamModelId {
    type Err = ModelIdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for CanonicalModelId {
    type Error = ModelIdError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for UpstreamModelId {
    type Error = ModelIdError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Built-in provider prefixes that cannot be stolen by additional accounts.
pub fn is_builtin_provider_prefix(prefix: &str) -> bool {
    BUILTIN_PROVIDER_PREFIXES
        .iter()
        .any(|candidate| *candidate == prefix)
}

/// Split on the first colon. The remainder is returned verbatim.
pub fn split_first_colon(raw: &str) -> Option<(&str, &str)> {
    let (left, right) = raw.split_once(':')?;
    if left.is_empty() || right.is_empty() {
        return None;
    }
    Some((left, right))
}

fn validate_model_id_str(raw: &str) -> Result<String, ModelIdError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ModelIdError::Empty);
    }
    if trimmed.len() > MAX_MODEL_ID_LEN {
        return Err(ModelIdError::TooLong { len: trimmed.len() });
    }
    if trimmed.chars().any(|ch| ch.is_control() || ch == '\0') {
        return Err(ModelIdError::ControlChar);
    }
    if trimmed.chars().any(|ch| ch.is_whitespace()) {
        return Err(ModelIdError::UnsafeIdentity);
    }
    reject_unsafe_model_identity(trimmed)?;
    Ok(trimmed.to_owned())
}

fn validate_safe_label(raw: &str, max_len: usize) -> Result<String, ModelIdError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ModelIdError::Empty);
    }
    if trimmed.len() > max_len {
        return Err(ModelIdError::TooLong { len: trimmed.len() });
    }
    if trimmed
        .chars()
        .any(|ch| ch.is_control() || ch == '\0' || ch.is_whitespace())
    {
        return Err(ModelIdError::ControlChar);
    }
    reject_unsafe_label(trimmed)?;
    Ok(trimmed.to_owned())
}

/// Accept provider slug punctuation including `@`, `%`, `?`, `#`.
/// Reject true URL/userinfo credentials and complete secret shapes.
fn reject_unsafe_model_identity(raw: &str) -> Result<(), ModelIdError> {
    if looks_like_complete_credential(raw) {
        return Err(ModelIdError::UnsafeIdentity);
    }
    Ok(())
}

fn reject_unsafe_label(raw: &str) -> Result<(), ModelIdError> {
    reject_unsafe_model_identity(raw)?;
    if raw.contains('/') {
        return Err(ModelIdError::UnsafeIdentity);
    }
    Ok(())
}

fn looks_like_complete_credential(raw: &str) -> bool {
    if raw.contains("://") || raw.contains('\\') {
        return true;
    }
    if looks_like_url_userinfo(raw) {
        return true;
    }
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("bearer ") || lower.contains(" authorization ") {
        return true;
    }
    if lower.starts_with("authorization:") || lower.starts_with("authorization ") {
        return true;
    }
    if has_openai_secret_shape(&lower) {
        return true;
    }
    if is_jwt_like(raw) {
        return true;
    }
    if lower.starts_with("api-key=")
        || lower.starts_with("api_key=")
        || lower.contains("api-key:")
        || lower.contains("api_key:")
    {
        return true;
    }
    if query_looks_like_credential(raw) {
        return true;
    }
    false
}

fn looks_like_url_userinfo(raw: &str) -> bool {
    let Some(at) = raw.find('@') else {
        return false;
    };
    let before = &raw[..at];
    let after = &raw[at + 1..];
    if after.is_empty() {
        return false;
    }
    // user:password@host — colon in pre-@ and no path separators.
    before.contains(':') && !before.contains('/')
}

fn query_looks_like_credential(raw: &str) -> bool {
    let Some(q) = raw.split_once('?').map(|(_, q)| q) else {
        return false;
    };
    let lower = q.to_ascii_lowercase();
    lower.contains("token=")
        || lower.contains("api_key=")
        || lower.contains("apikey=")
        || lower.contains("access_token=")
        || lower.contains("secret=")
        || lower.contains("password=")
        || lower.contains("bearer=")
}

fn has_openai_secret_shape(lower: &str) -> bool {
    let mut rest = lower;
    loop {
        let Some(idx) = rest.find("sk-") else {
            return false;
        };
        let at_boundary = idx == 0 || !rest.as_bytes()[idx - 1].is_ascii_alphanumeric();
        if at_boundary {
            let after = &rest[idx + 3..];
            let body = after.strip_prefix("proj-").unwrap_or(after);
            let body_len = body
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .count();
            if body_len >= 16 {
                return true;
            }
        }
        rest = &rest[idx + 3..];
        if rest.is_empty() {
            return false;
        }
    }
}

fn is_jwt_like(raw: &str) -> bool {
    let parts: Vec<&str> = raw.split('.').collect();
    if parts.len() != 3 || raw.len() < 36 {
        return false;
    }
    let is_b64url = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            && s.len() >= 8
    };
    if !parts.iter().all(|p| is_b64url(p)) {
        return false;
    }
    parts[0].to_ascii_lowercase().starts_with("eyj")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_accepts_curated_and_user_keys() {
        let curated = CanonicalModelId::new("openai-gpt-5.6-sol").unwrap();
        assert_eq!(curated.as_str(), "openai-gpt-5.6-sol");
        assert!(curated.split_provider_prefix().is_none());
        assert!(curated.is_reserved_compatibility_selection());

        let user = CanonicalModelId::new("my-local").unwrap();
        assert!(user.is_reserved_compatibility_selection());
    }

    #[test]
    fn canonical_colon_split_is_first_colon_only() {
        let id = CanonicalModelId::new("openai:gpt-4o:preview").unwrap();
        assert_eq!(
            id.split_provider_prefix(),
            Some(("openai", "gpt-4o:preview"))
        );
        assert!(id.is_builtin_namespaced());
    }

    #[test]
    fn discovered_id_uses_first_colon_separator() {
        let upstream = UpstreamModelId::new("openai/gpt-5.6-sol").unwrap();
        let id = CanonicalModelId::discovered("work-openai", &upstream).unwrap();
        assert_eq!(id.as_str(), "work-openai:openai/gpt-5.6-sol");
        assert!(!id.is_reserved_compatibility_selection());
    }

    #[test]
    fn upstream_preserves_arbitrary_valid_strings() {
        for raw in [
            "gpt-5.6-sol",
            "openai/gpt-5.6-sol",
            "org.model:v1.2.3",
            "org/model@rev",
            "org/model%2Frevision",
            "model?variant=fast",
            "model#snapshot",
        ] {
            let id = UpstreamModelId::new(raw).unwrap();
            assert_eq!(id.as_str(), raw);
        }
    }

    #[test]
    fn accepts_legitimate_model_names_that_share_secret_substrings() {
        for raw in ["flask-v1", "mask-large", "whiskey-jam", "bearer-model"] {
            CanonicalModelId::new(raw).unwrap();
            UpstreamModelId::new(raw).unwrap();
        }
        let upstream = UpstreamModelId::new("org/model@rev").unwrap();
        let discovered = CanonicalModelId::discovered("openai", &upstream).unwrap();
        assert_eq!(discovered.as_str(), "openai:org/model@rev");
    }

    #[test]
    fn rejects_empty_controls_length_and_secrets() {
        assert_eq!(CanonicalModelId::new("").unwrap_err(), ModelIdError::Empty);
        assert_eq!(
            CanonicalModelId::new("bad\nid").unwrap_err(),
            ModelIdError::ControlChar
        );
        assert!(matches!(
            CanonicalModelId::new("a".repeat(MAX_MODEL_ID_LEN + 1)),
            Err(ModelIdError::TooLong { .. })
        ));
        assert_eq!(
            UpstreamModelId::new("sk-abcdefghijklmnopqrst").unwrap_err(),
            ModelIdError::UnsafeIdentity
        );
        assert_eq!(
            UpstreamModelId::new("Bearer tokensecretvalue").unwrap_err(),
            ModelIdError::UnsafeIdentity
        );
        assert_eq!(
            CanonicalModelId::new("https://api.example.com/v1").unwrap_err(),
            ModelIdError::UnsafeIdentity
        );
        assert!(CanonicalModelId::new("user@host").is_ok());
        assert_eq!(
            CanonicalModelId::new("user:pass@host").unwrap_err(),
            ModelIdError::UnsafeIdentity
        );
        assert_eq!(
            CanonicalModelId::new("model?api_key=secret").unwrap_err(),
            ModelIdError::UnsafeIdentity
        );
        assert_eq!(
            CanonicalModelId::new("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.signaturepart")
                .unwrap_err(),
            ModelIdError::UnsafeIdentity
        );
    }

    #[test]
    fn serde_is_transparent_string() {
        let id = CanonicalModelId::new("openai-gpt-5.6-terra").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"openai-gpt-5.6-terra\"");
        let back: CanonicalModelId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn provenance_round_trip_and_live_match() {
        let upstream = UpstreamModelId::new("gpt-5.6-sol").unwrap();
        let provenance = ModelRouteProvenance::new(
            "openai",
            Some("01234567-89ab-cdef-0123-456789abcdef"),
            Some("openai"),
            Some("openai_platform"),
            &upstream,
            3,
        )
        .unwrap();
        let json = serde_json::to_string(&provenance).unwrap();
        let back: ModelRouteProvenance = serde_json::from_str(&json).unwrap();
        assert_eq!(back, provenance);
        assert!(back.matches_live(
            Some("openai"),
            Some("01234567-89ab-cdef-0123-456789abcdef"),
            Some("openai"),
            Some("openai_platform"),
            Some("gpt-5.6-sol"),
            Some(3),
        ));
        assert!(!back.matches_live(
            Some("work-openai"),
            Some("01234567-89ab-cdef-0123-456789abcdef"),
            Some("openai"),
            Some("openai_platform"),
            Some("gpt-5.6-sol"),
            Some(3),
        ));
        assert!(!back.matches_live(
            Some("openai"),
            Some("01234567-89ab-cdef-0123-456789abcdef"),
            Some("openai"),
            Some("openai_platform"),
            Some("gpt-5.6-sol"),
            Some(9),
        ));
    }

    #[test]
    fn exact_constructor_rejects_generation_zero() {
        let upstream = UpstreamModelId::new("gpt-4o").unwrap();
        assert_eq!(
            ModelRouteProvenance::new(
                "openai",
                Some("01234567-89ab-cdef-0123-456789abcdef"),
                Some("openai"),
                Some("openai_platform"),
                &upstream,
                0,
            )
            .unwrap_err(),
            ModelIdError::InvalidGeneration
        );
    }

    #[test]
    fn old_json_without_provenance_deserializes() {
        // Transparent string types only — no nested provenance required.
        let id: CanonicalModelId = serde_json::from_str("\"grok-4.5\"").unwrap();
        assert_eq!(id.as_str(), "grok-4.5");
    }

    #[test]
    fn debug_does_not_include_secret_shaped_values() {
        let id = CanonicalModelId::new("grok-4.5").unwrap();
        let rendered = format!("{id:?}");
        assert!(rendered.contains("grok-4.5"));
        assert!(!rendered.contains("sk-"));
    }
}
