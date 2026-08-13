//! Credential-free sidecar identity types for provider instances.
//!
//! These types describe *what* a provider instance is (its upstream kind, the
//! request surfaces it exposes, and how credentials are expected to be routed).
//! They are the canonical metadata/service foundation for the upcoming
//! multi-account platform and remain strictly additive: no existing public
//! enum, tag, order, or constructor changes.
//!
//! Secret semantics are precise: no credential material is read or copied from
//! the vault, environment, or an auth helper; an explicit `api_key` value and
//! an inline auth-helper command are always omitted from these types.
//! Arbitrary user-authored provider policy/plugin JSON (for example the
//! OpenRouter plugin list) is preserved verbatim as local configuration and
//! must be treated as potentially sensitive — never logged or sent to external
//! telemetry. `auth_scheme` retains only the safe string spelling (for example
//! `x_api_key`), never the credential itself.

use super::id::{BuiltInProviderId, ProviderId, ProviderRef};
use crate::agent::model_providers::{OpenRouterPlugin, OpenRouterProviderPreferences};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Canonical textual length of a UUID (`8-4-4-4-12`).
pub const MAX_INCARNATION_LEN: usize = 36;

/// Validated opaque stable value identifying one incarnation of a provider
/// instance. Suitable for persisted lifecycle state (for example "this
/// descriptor corresponds to generation N of provider P").
///
/// The value is a canonical UUID textual form (`8-4-4-4-12`, hexadecimal only)
/// so it cannot carry a path, URL, or credential-like payload. Deserialization
/// is strict: any non-canonical form is rejected rather than silently accepted.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ProviderIncarnation(String);

impl ProviderIncarnation {
    /// Validate and construct a canonical UUID-form token.
    pub fn new(raw: impl AsRef<str>) -> Result<Self, IncarnationError> {
        let s = raw.as_ref();
        if s.is_empty() {
            return Err(IncarnationError::Empty);
        }
        if s.len() != MAX_INCARNATION_LEN {
            return Err(IncarnationError::InvalidLength { len: s.len() });
        }
        let bytes = s.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            match i {
                8 | 13 | 18 | 23 => {
                    if b != b'-' {
                        return Err(IncarnationError::InvalidChars { ch: b as char });
                    }
                }
                _ => {
                    if !b.is_ascii_hexdigit() {
                        return Err(IncarnationError::InvalidChars { ch: b as char });
                    }
                }
            }
        }
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for ProviderIncarnation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ProviderIncarnation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for ProviderIncarnation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validation failure for a [`ProviderIncarnation`] token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncarnationError {
    Empty,
    InvalidLength { len: usize },
    InvalidChars { ch: char },
}

impl std::fmt::Display for IncarnationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "incarnation token is empty"),
            Self::InvalidLength { len } => write!(
                f,
                "incarnation token length {len} is not the canonical UUID length {MAX_INCARNATION_LEN}"
            ),
            Self::InvalidChars { ch } => write!(
                f,
                "incarnation token contains invalid character `{ch}` (canonical 8-4-4-4-12 UUID only)"
            ),
        }
    }
}

impl std::error::Error for IncarnationError {}

/// Canonical upstream provider kind for a provider instance.
///
/// This is a credential/session-free sidecar classification separate from
/// [`crate::agent::model_providers::ModelProviderKind`]; it converts from that
/// existing kind without replacing it. Callers that already encode the kind on
/// a resolved model keep using `ModelProviderKind` unchanged.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Xai,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "openrouter")]
    OpenRouter,
    Anthropic,
    #[default]
    #[serde(rename = "openai_compatible")]
    OpenAiCompatible,
    Zai,
}

impl ProviderKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Xai => "xai",
            Self::OpenAi => "openai",
            Self::OpenRouter => "openrouter",
            Self::Anthropic => "anthropic",
            Self::OpenAiCompatible => "openai_compatible",
            Self::Zai => "zai",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Xai => "xAI",
            Self::OpenAi => "OpenAI",
            Self::OpenRouter => "OpenRouter",
            Self::Anthropic => "Anthropic",
            Self::OpenAiCompatible => "OpenAI Compatible",
            Self::Zai => "Z.ai",
        }
    }

    pub const fn is_openai_compatible_family(self) -> bool {
        matches!(
            self,
            Self::OpenAiCompatible | Self::OpenAi | Self::OpenRouter | Self::Zai
        )
    }
}

impl From<BuiltInProviderId> for ProviderKind {
    fn from(b: BuiltInProviderId) -> Self {
        match b {
            BuiltInProviderId::Xai => Self::Xai,
            BuiltInProviderId::OpenAi => Self::OpenAi,
            BuiltInProviderId::OpenRouter => Self::OpenRouter,
            BuiltInProviderId::Anthropic => Self::Anthropic,
        }
    }
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The request surface a provider instance exposes.
///
/// This classifies how models are reached (an OpenAI-compatible platform,
/// native OpenRouter routing, ChatGPT subscription inference, a compatible
/// subset, direct Anthropic Messages, or read/retrieval only) independently of
/// any credential. It is backward-compatibility metadata only in this PR;
/// nothing routes inference from it yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiSurface {
    #[serde(rename = "openai_platform")]
    OpenAiPlatform,
    #[serde(rename = "openrouter_native")]
    OpenRouterNative,
    #[serde(rename = "chatgpt_inference")]
    ChatGptInference,
    #[serde(rename = "openai_compatible_subset")]
    OpenAiCompatibleSubset,
    RetrievalOnly,
    #[serde(rename = "anthropic_messages")]
    AnthropicMessages,
}

impl ApiSurface {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiPlatform => "openai_platform",
            Self::OpenRouterNative => "openrouter_native",
            Self::ChatGptInference => "chatgpt_inference",
            Self::OpenAiCompatibleSubset => "openai_compatible_subset",
            Self::RetrievalOnly => "retrieval_only",
            Self::AnthropicMessages => "anthropic_messages",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::OpenAiPlatform => "OpenAI Platform",
            Self::OpenRouterNative => "OpenRouter Native",
            Self::ChatGptInference => "ChatGPT Inference",
            Self::OpenAiCompatibleSubset => "OpenAI-Compatible Subset",
            Self::RetrievalOnly => "Retrieval-Only",
            Self::AnthropicMessages => "Anthropic Messages",
        }
    }
}

impl std::fmt::Display for ApiSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a provider instance expects credentials to be routed.
///
/// Describes only the *route* a credential takes — never the credential
/// itself. `None` means the instance operates with no credential (BYOK-less
/// public or local endpoints).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRoute {
    XaiSession,
    ApiKey,
    #[serde(rename = "openai_platform")]
    OpenAiPlatform,
    #[serde(rename = "chatgpt_oauth")]
    ChatGptOauth,
    AuthHelper,
    None,
}

impl CredentialRoute {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::XaiSession => "xai_session",
            Self::ApiKey => "api_key",
            Self::OpenAiPlatform => "openai_platform",
            Self::ChatGptOauth => "chatgpt_oauth",
            Self::AuthHelper => "auth_helper",
            Self::None => "none",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::XaiSession => "xAI Session",
            Self::ApiKey => "API Key",
            Self::OpenAiPlatform => "OpenAI Platform",
            Self::ChatGptOauth => "ChatGPT OAuth",
            Self::AuthHelper => "Auth Helper",
            Self::None => "None",
        }
    }
}

impl std::fmt::Display for CredentialRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One supported request/credential pairing for a provider instance.
///
/// A product descriptor may support more than one route (for example a built-in
/// OpenAI instance serves both an OpenAI Platform/API-key route and a ChatGPT
/// inference/OAuth route), so instances model an *ordered route set* rather
/// than a single scalar. This type is credential-free.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRouteDescriptor {
    pub api_surface: ApiSurface,
    pub credential_route: CredentialRoute,
}

impl ProviderRouteDescriptor {
    pub const fn new(api_surface: ApiSurface, credential_route: CredentialRoute) -> Self {
        Self {
            api_surface,
            credential_route,
        }
    }
}

fn default_enabled() -> bool {
    true
}

/// Credential-free descriptor for one provider instance.
///
/// This is the canonical safe metadata for a provider instance: it identifies
/// the instance and its upstream kind, the ordered set of request/credential
/// routes it supports, its display name, and its base/admin URLs. It also
/// carries safe config state not representable in
/// [`crate::provider_registry::lifecycle::ProviderMetadata`] (full env-key
/// list, API backend, raw auth-scheme spelling, auth-helper reference,
/// OpenRouter policy).
///
/// No credential material is ever read or copied from the vault, environment,
/// or an auth helper, and an explicit `api_key` value or inline auth-helper
/// command is always omitted. Preserved OpenRouter policy/plugin JSON is
/// user-authored local config and must be treated as potentially sensitive
/// (never logged or sent to external telemetry).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderInstanceDescriptor {
    pub id: ProviderId,
    pub provider_ref: ProviderRef,
    pub kind: ProviderKind,
    /// Supported routes in preference order (first is primary).
    #[serde(default)]
    pub routes: Vec<ProviderRouteDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_base_url: Option<String>,
    /// Optional persisted lifecycle token. Canonical UUID, secret-free.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<ProviderIncarnation>,
    /// Full configured environment-variable key list, priority order. Safe
    /// (names only), distinct from the single-name metadata `env_key`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_keys: Vec<String>,
    /// API backend (chat_completions / responses / messages) as a canonical
    /// string. Not representable in `ProviderMetadata`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_backend: Option<String>,
    /// Exact safe auth-scheme spelling from config (`x_api_key`,
    /// `x-api-key`, `xapikey`, `bearer`, `none`, `custom_header`). Retained so
    /// later consumers can reproduce the exact wire scheme without the mapping
    /// in `ProviderMetadata` (which cannot represent every spelling).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_scheme: Option<String>,
    /// Referenced auth-helper provider name. Safe identifier, never a secret.
    /// Synthesized as `model_provider:<id>` for inline auth helpers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openrouter_fallback_models: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openrouter_provider_preferences: Option<OpenRouterProviderPreferences>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openrouter_plugins: Vec<OpenRouterPlugin>,
    #[serde(default)]
    pub openrouter_pacing: bool,
}

impl ProviderInstanceDescriptor {
    /// Effective display label, falling back to the configured slug.
    pub fn display_label(&self) -> &str {
        self.display_name
            .as_deref()
            .unwrap_or_else(|| self.id.as_str())
    }

    /// The primary (first) supported route, if any.
    pub fn primary_route(&self) -> Option<&ProviderRouteDescriptor> {
        self.routes.first()
    }

    /// Whether the instance advertises the given surface/route pairing.
    pub fn has_route(&self, api_surface: ApiSurface, credential_route: CredentialRoute) -> bool {
        self.routes
            .iter()
            .any(|r| r.api_surface == api_surface && r.credential_route == credential_route)
    }

    /// Whether the instance advertises a route using the given credential
    /// route, across all its surfaces.
    pub fn has_credential_route(&self, route: CredentialRoute) -> bool {
        self.routes.iter().any(|r| r.credential_route == route)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::model_providers::ModelProviderKind;
    use serde_json::json;

    #[test]
    fn kind_serde_emits_canonical_strings() {
        let cases = [
            (ProviderKind::Xai, "xai"),
            (ProviderKind::OpenAi, "openai"),
            (ProviderKind::OpenRouter, "openrouter"),
            (ProviderKind::Anthropic, "anthropic"),
            (ProviderKind::OpenAiCompatible, "openai_compatible"),
            (ProviderKind::Zai, "zai"),
        ];
        for (kind, expected) in cases {
            assert_eq!(
                serde_json::to_string(&kind).unwrap(),
                format!("\"{expected}\"")
            );
            let back: ProviderKind = serde_json::from_value(json!(expected)).unwrap();
            assert_eq!(back, kind, "round-trip for {expected}");
        }
        assert_eq!(ProviderKind::default(), ProviderKind::OpenAiCompatible);
        assert_eq!(
            ProviderKind::from(BuiltInProviderId::OpenAi),
            ProviderKind::OpenAi
        );
    }

    #[test]
    fn kind_maps_from_model_provider_kind() {
        for (model, sidecar) in [
            (ModelProviderKind::Xai, ProviderKind::Xai),
            (ModelProviderKind::OpenAi, ProviderKind::OpenAi),
            (ModelProviderKind::OpenRouter, ProviderKind::OpenRouter),
            (ModelProviderKind::Anthropic, ProviderKind::Anthropic),
            (ModelProviderKind::Zai, ProviderKind::Zai),
            (
                ModelProviderKind::OpenAiCompatible,
                ProviderKind::OpenAiCompatible,
            ),
        ] {
            assert_eq!(ProviderKind::from(model), sidecar);
        }
    }

    #[test]
    fn surface_and_route_serde_match_canonical_strings() {
        for (surface, expected) in [
            (ApiSurface::OpenAiPlatform, "openai_platform"),
            (ApiSurface::OpenRouterNative, "openrouter_native"),
            (ApiSurface::ChatGptInference, "chatgpt_inference"),
            (
                ApiSurface::OpenAiCompatibleSubset,
                "openai_compatible_subset",
            ),
            (ApiSurface::RetrievalOnly, "retrieval_only"),
            (ApiSurface::AnthropicMessages, "anthropic_messages"),
        ] {
            assert_eq!(
                serde_json::to_string(&surface).unwrap(),
                format!("\"{expected}\"")
            );
            let back: ApiSurface = serde_json::from_value(json!(expected)).unwrap();
            assert_eq!(back, surface);
        }
        for (route, expected) in [
            (CredentialRoute::XaiSession, "xai_session"),
            (CredentialRoute::ApiKey, "api_key"),
            (CredentialRoute::OpenAiPlatform, "openai_platform"),
            (CredentialRoute::ChatGptOauth, "chatgpt_oauth"),
            (CredentialRoute::AuthHelper, "auth_helper"),
            (CredentialRoute::None, "none"),
        ] {
            assert_eq!(
                serde_json::to_string(&route).unwrap(),
                format!("\"{expected}\"")
            );
            let back: CredentialRoute = serde_json::from_value(json!(expected)).unwrap();
            assert_eq!(back, route);
        }
    }

    #[test]
    fn incarnation_accepts_canonical_uuid() {
        let ok = ProviderIncarnation::new("123e4567-e89b-12d3-a456-426614174000");
        assert!(ok.is_ok());
        assert_eq!(ok.unwrap().as_str(), "123e4567-e89b-12d3-a456-426614174000");
    }

    #[test]
    fn incarnation_rejects_non_uuid_forms() {
        // Too short / too long.
        assert!(matches!(
            ProviderIncarnation::new("abcd"),
            Err(IncarnationError::InvalidLength { .. })
        ));
        assert!(matches!(
            ProviderIncarnation::new("123e4567-e89b-12d3-a456-4266141740000"),
            Err(IncarnationError::InvalidLength { .. })
        ));
        // Path / URL separators and non-hex are rejected.
        assert!(matches!(
            ProviderIncarnation::new("123e4567-e89b-12d3-a456/426614174000"),
            Err(IncarnationError::InvalidChars { .. })
        ));
        assert!(matches!(
            ProviderIncarnation::new("123e4567:e89b:12d3:a456:426614174000"),
            Err(IncarnationError::InvalidChars { .. })
        ));
        assert!(matches!(
            ProviderIncarnation::new("zz3e4567-e89b-12d3-a456-426614174000"),
            Err(IncarnationError::InvalidChars { .. })
        ));
    }

    #[test]
    fn incarnation_strict_serde_rejects_invalid() {
        let valid = "123e4567-e89b-12d3-a456-426614174000";
        let round: ProviderIncarnation = serde_json::from_value(json!(valid)).unwrap();
        assert_eq!(round.as_str(), valid);
        // A path-like value must not deserialize.
        assert!(serde_json::from_value::<ProviderIncarnation>(json!("etc/passwd")).is_err());
        assert!(serde_json::from_value::<ProviderIncarnation>(json!("sk-abcdef")).is_err());
        assert!(serde_json::from_value::<ProviderIncarnation>(json!(42)).is_err());
    }

    #[test]
    fn route_set_and_filters_work() {
        let desc = dummy_descriptor(
            ProviderKind::Xai,
            vec![
                ProviderRouteDescriptor::new(
                    ApiSurface::OpenAiCompatibleSubset,
                    CredentialRoute::XaiSession,
                ),
                ProviderRouteDescriptor::new(
                    ApiSurface::OpenAiCompatibleSubset,
                    CredentialRoute::ApiKey,
                ),
            ],
        );
        assert_eq!(
            desc.primary_route().unwrap().credential_route,
            CredentialRoute::XaiSession
        );
        assert!(desc.has_route(ApiSurface::OpenAiCompatibleSubset, CredentialRoute::ApiKey));
        assert!(!desc.has_route(ApiSurface::OpenAiPlatform, CredentialRoute::ApiKey));
        assert!(desc.has_credential_route(CredentialRoute::ApiKey));
        assert!(!desc.has_credential_route(CredentialRoute::ChatGptOauth));
    }

    #[test]
    fn descriptor_is_secret_free_and_round_trips() {
        let mut desc = dummy_descriptor(
            ProviderKind::OpenAiCompatible,
            vec![ProviderRouteDescriptor::new(
                ApiSurface::OpenAiCompatibleSubset,
                CredentialRoute::ApiKey,
            )],
        );
        desc.display_name = Some("Local".into());
        desc.base_url = Some("http://127.0.0.1:8080/v1".into());
        desc.env_keys = vec!["LOCAL_API_KEY".into()];
        desc.incarnation =
            Some(ProviderIncarnation::new("123e4567-e89b-12d3-a456-426614174000").unwrap());

        // Debug / serde must not print credential *values* (names exist only
        // as config key references, not secrets).
        let debug = format!("{desc:?}");
        assert!(!debug.contains("sk-") && !debug.contains("Bearer token"));
        let json = serde_json::to_string(&desc).unwrap();
        assert!(!json.contains("sk-"), "no secret-looking value: {json}");
        let back: ProviderInstanceDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(back, desc);
    }

    fn dummy_descriptor(
        kind: ProviderKind,
        routes: Vec<ProviderRouteDescriptor>,
    ) -> ProviderInstanceDescriptor {
        let id = ProviderId::new("dummy").unwrap();
        ProviderInstanceDescriptor {
            provider_ref: ProviderRef::Configured(id.clone()),
            id,
            kind,
            routes,
            display_name: None,
            enabled: true,
            base_url: None,
            admin_base_url: None,
            incarnation: None,
            env_keys: Vec::new(),
            api_backend: None,
            auth_scheme: None,
            auth_provider: None,
            openrouter_fallback_models: Vec::new(),
            openrouter_provider_preferences: None,
            openrouter_plugins: Vec::new(),
            openrouter_pacing: false,
        }
    }
}
