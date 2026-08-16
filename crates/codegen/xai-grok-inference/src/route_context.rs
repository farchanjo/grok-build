//! Credential-free provider route context carried on the sampler sidecar.
//!
//! Identifies the exact provider instance, credential route, binding generation,
//! and authority of a request. Never carries secrets, tokens, or PII.
//! Deserialization always reconstructs through the validated builder.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Upstream provider kind for route partitioning and policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteProviderKind {
    Xai,
    OpenAi,
    OpenRouter,
    Anthropic,
    OpenAiCompatible,
    Zai,
    Custom,
}

impl RouteProviderKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Xai => "xai",
            Self::OpenAi => "openai",
            Self::OpenRouter => "openrouter",
            Self::Anthropic => "anthropic",
            Self::OpenAiCompatible => "openai_compatible",
            Self::Zai => "zai",
            Self::Custom => "custom",
        }
    }

    pub const fn is_openrouter(self) -> bool {
        matches!(self, Self::OpenRouter)
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "xai" => Some(Self::Xai),
            "openai" => Some(Self::OpenAi),
            "openrouter" => Some(Self::OpenRouter),
            "anthropic" => Some(Self::Anthropic),
            "openai_compatible" => Some(Self::OpenAiCompatible),
            "zai" => Some(Self::Zai),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// Request surface the route targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteApiSurface {
    OpenAiPlatform,
    OpenRouterNative,
    ChatGptInference,
    OpenAiCompatibleSubset,
    AnthropicMessages,
    RetrievalOnly,
}

impl RouteApiSurface {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiPlatform => "openai_platform",
            Self::OpenRouterNative => "openrouter_native",
            Self::ChatGptInference => "chatgpt_inference",
            Self::OpenAiCompatibleSubset => "openai_compatible_subset",
            Self::AnthropicMessages => "anthropic_messages",
            Self::RetrievalOnly => "retrieval_only",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "openai_platform" => Some(Self::OpenAiPlatform),
            "openrouter_native" => Some(Self::OpenRouterNative),
            "chatgpt_inference" => Some(Self::ChatGptInference),
            "openai_compatible_subset" => Some(Self::OpenAiCompatibleSubset),
            "anthropic_messages" => Some(Self::AnthropicMessages),
            "retrieval_only" => Some(Self::RetrievalOnly),
            _ => None,
        }
    }
}

/// Credential route spelling — never the credential itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteCredentialRoute {
    XaiSession,
    ApiKey,
    #[serde(rename = "openai_platform")]
    OpenAiPlatform,
    #[serde(rename = "chatgpt_oauth")]
    ChatGptOauth,
    AuthHelper,
    None,
}

impl RouteCredentialRoute {
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

    pub const fn uses_xai_session(self) -> bool {
        matches!(self, Self::XaiSession)
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "xai_session" => Some(Self::XaiSession),
            "api_key" => Some(Self::ApiKey),
            "openai_platform" => Some(Self::OpenAiPlatform),
            "chatgpt_oauth" => Some(Self::ChatGptOauth),
            "auth_helper" => Some(Self::AuthHelper),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

/// Whether the route has a proven durable binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RouteAuthority {
    /// Structured route with a proven current binding.
    Authoritative,
    /// Structured route without a proven binding (missing/stale/unproven).
    Unverified,
    /// Legacy host-derived fallback only (not a structured repairable route).
    #[default]
    HostFallback,
}

impl RouteAuthority {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authoritative => "authoritative",
            Self::Unverified => "unverified",
            Self::HostFallback => "host_fallback",
        }
    }

    pub fn from_wire(s: &str) -> Option<Self> {
        match s {
            "authoritative" => Some(Self::Authoritative),
            "unverified" => Some(Self::Unverified),
            "host_fallback" => Some(Self::HostFallback),
            _ => None,
        }
    }

    /// Exact repair is only enabled for authoritative structured routes.
    pub const fn is_authoritative(self) -> bool {
        matches!(self, Self::Authoritative)
    }

    pub const fn is_non_authoritative(self) -> bool {
        !self.is_authoritative()
    }
}

/// Optional pacing override attached to a route context.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RoutePacingOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_interval_ms: Option<u64>,
}

/// Wire form used only during serde; never trusted as validated state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RawProviderRouteContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    instance_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    api_surface: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    credential_route: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    incarnation: Option<String>,
    #[serde(default)]
    registry_generation: u64,
    #[serde(default)]
    binding_generation: u64,
    #[serde(default)]
    credential_generation: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pacing: Option<RoutePacingOverride>,
}

/// Validated, credential-free route identity for one inference turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRouteContext {
    instance_id: String,
    provider_kind: RouteProviderKind,
    api_surface: RouteApiSurface,
    credential_route: RouteCredentialRoute,
    incarnation: Option<String>,
    registry_generation: u64,
    binding_generation: u64,
    credential_generation: u64,
    authority: RouteAuthority,
    origin: Option<String>,
    model_partition: Option<String>,
    pacing: RoutePacingOverride,
}

impl ProviderRouteContext {
    pub fn builder() -> ProviderRouteContextBuilder {
        ProviderRouteContextBuilder::default()
    }

    /// Legacy host-derived context: HostFallback authority only.
    pub fn legacy_from_config(config: &crate::config::InferenceConfig) -> Self {
        let kind = match config.provider_identity {
            crate::config::ProviderIdentity::Xai => RouteProviderKind::Xai,
            crate::config::ProviderIdentity::OpenAi => RouteProviderKind::OpenAi,
            crate::config::ProviderIdentity::OpenRouter => RouteProviderKind::OpenRouter,
            crate::config::ProviderIdentity::Anthropic => RouteProviderKind::Anthropic,
            crate::config::ProviderIdentity::Custom => RouteProviderKind::Custom,
        };
        let (api_surface, credential_route) = default_surface_and_route(kind);
        let authority = if matches!(kind, RouteProviderKind::Custom) {
            RouteAuthority::HostFallback
        } else {
            // Identity present but no structured binding proof → Unverified.
            RouteAuthority::Unverified
        };
        Self::builder()
            .instance_id(kind.as_str())
            .provider_kind(kind)
            .api_surface(api_surface)
            .credential_route(credential_route)
            .authority(authority)
            .origin_from_base_url(config.base_url.as_str())
            .model_partition(config.model.as_str())
            .build()
            .unwrap_or_else(|_| {
                // Absolute last resort: explicit host fallback.
                Self {
                    instance_id: "custom".into(),
                    provider_kind: RouteProviderKind::Custom,
                    api_surface: RouteApiSurface::OpenAiCompatibleSubset,
                    credential_route: RouteCredentialRoute::None,
                    incarnation: None,
                    registry_generation: 0,
                    binding_generation: 0,
                    credential_generation: 0,
                    authority: RouteAuthority::HostFallback,
                    origin: origin_from_url(config.base_url.as_str()),
                    model_partition: Some(config.model.clone()),
                    pacing: RoutePacingOverride::default(),
                }
            })
    }

    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }
    pub fn provider_kind(&self) -> RouteProviderKind {
        self.provider_kind
    }
    pub fn api_surface(&self) -> RouteApiSurface {
        self.api_surface
    }
    pub fn credential_route(&self) -> RouteCredentialRoute {
        self.credential_route
    }
    pub fn incarnation(&self) -> Option<&str> {
        self.incarnation.as_deref()
    }
    pub fn registry_generation(&self) -> u64 {
        self.registry_generation
    }
    pub fn binding_generation(&self) -> u64 {
        self.binding_generation
    }
    pub fn credential_generation(&self) -> u64 {
        self.credential_generation
    }
    pub fn authority(&self) -> RouteAuthority {
        self.authority
    }
    pub fn origin(&self) -> Option<&str> {
        self.origin.as_deref()
    }
    pub fn model_partition(&self) -> Option<&str> {
        self.model_partition.as_deref()
    }
    pub fn pacing(&self) -> &RoutePacingOverride {
        &self.pacing
    }

    /// Stable pacing partition key fragment: instance + route + incarnation.
    pub fn pacing_partition_key(&self) -> String {
        format!(
            "{}|{}|{}",
            self.instance_id,
            self.credential_route.as_str(),
            self.incarnation.as_deref().unwrap_or("-")
        )
    }
}

impl Serialize for ProviderRouteContext {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let raw = RawProviderRouteContext {
            instance_id: Some(self.instance_id.clone()),
            kind_label: Some(self.provider_kind.as_str().to_owned()),
            api_surface: Some(self.api_surface.as_str().to_owned()),
            credential_route: Some(self.credential_route.as_str().to_owned()),
            incarnation: self.incarnation.clone(),
            registry_generation: self.registry_generation,
            binding_generation: self.binding_generation,
            credential_generation: self.credential_generation,
            authority: Some(self.authority.as_str().to_owned()),
            origin: self.origin.clone(),
            model_partition: self.model_partition.clone(),
            pacing: if self.pacing == RoutePacingOverride::default() {
                None
            } else {
                Some(self.pacing.clone())
            },
        };
        raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProviderRouteContext {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawProviderRouteContext::deserialize(deserializer)?;
        let kind = raw
            .kind_label
            .as_deref()
            .and_then(RouteProviderKind::parse)
            .ok_or_else(|| serde::de::Error::custom("unknown or missing kind_label"))?;
        let route = raw
            .credential_route
            .as_deref()
            .and_then(RouteCredentialRoute::parse)
            .ok_or_else(|| serde::de::Error::custom("unknown or missing credential_route"))?;
        let surface = raw
            .api_surface
            .as_deref()
            .and_then(RouteApiSurface::parse)
            .unwrap_or_else(|| default_surface_and_route(kind).0);
        let authority = raw
            .authority
            .as_deref()
            .and_then(RouteAuthority::from_wire)
            .ok_or_else(|| serde::de::Error::custom("unknown or missing authority"))?;
        let instance_id = raw
            .instance_id
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kind.as_str().to_owned());
        let mut b = ProviderRouteContext::builder()
            .instance_id(instance_id)
            .provider_kind(kind)
            .api_surface(surface)
            .credential_route(route)
            .registry_generation(raw.registry_generation)
            .binding_generation(raw.binding_generation)
            .credential_generation(raw.credential_generation)
            .authority(authority);
        if let Some(inc) = raw.incarnation.filter(|s| !s.is_empty()) {
            b = b.incarnation(inc);
        }
        if let Some(o) = raw.origin {
            b = b.origin(o);
        }
        if let Some(m) = raw.model_partition {
            b = b.model_partition(m);
        }
        if let Some(p) = raw.pacing {
            b = b.pacing(p);
        }
        b.build().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProviderRouteContextBuilder {
    instance_id: Option<String>,
    provider_kind: Option<RouteProviderKind>,
    api_surface: Option<RouteApiSurface>,
    credential_route: Option<RouteCredentialRoute>,
    incarnation: Option<String>,
    registry_generation: u64,
    binding_generation: u64,
    credential_generation: u64,
    authority: Option<RouteAuthority>,
    origin: Option<String>,
    model_partition: Option<String>,
    pacing: RoutePacingOverride,
}

impl ProviderRouteContextBuilder {
    pub fn instance_id(mut self, id: impl Into<String>) -> Self {
        self.instance_id = Some(id.into());
        self
    }
    pub fn provider_kind(mut self, kind: RouteProviderKind) -> Self {
        self.provider_kind = Some(kind);
        self
    }
    pub fn api_surface(mut self, surface: RouteApiSurface) -> Self {
        self.api_surface = Some(surface);
        self
    }
    pub fn credential_route(mut self, route: RouteCredentialRoute) -> Self {
        self.credential_route = Some(route);
        self
    }
    pub fn incarnation(mut self, inc: impl Into<String>) -> Self {
        let s = inc.into();
        if !s.is_empty() {
            self.incarnation = Some(s);
        }
        self
    }
    pub fn registry_generation(mut self, g: u64) -> Self {
        self.registry_generation = g;
        self
    }
    pub fn binding_generation(mut self, g: u64) -> Self {
        self.binding_generation = g;
        self
    }
    pub fn credential_generation(mut self, g: u64) -> Self {
        self.credential_generation = g;
        self
    }
    pub fn authority(mut self, a: RouteAuthority) -> Self {
        self.authority = Some(a);
        self
    }
    pub fn origin(mut self, o: impl Into<String>) -> Self {
        self.origin = Some(o.into());
        self
    }
    pub fn origin_from_base_url(mut self, base_url: &str) -> Self {
        self.origin = origin_from_url(base_url);
        self
    }
    pub fn model_partition(mut self, m: impl Into<String>) -> Self {
        self.model_partition = Some(m.into());
        self
    }
    pub fn pacing(mut self, p: RoutePacingOverride) -> Self {
        self.pacing = p;
        self
    }

    pub fn build(self) -> Result<ProviderRouteContext, String> {
        let instance_id = self
            .instance_id
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "instance_id required".to_owned())?;
        let provider_kind = self
            .provider_kind
            .ok_or_else(|| "provider_kind required".to_owned())?;
        let api_surface = self
            .api_surface
            .ok_or_else(|| "api_surface required".to_owned())?;
        let credential_route = self
            .credential_route
            .ok_or_else(|| "credential_route required".to_owned())?;
        let authority = self
            .authority
            .ok_or_else(|| "authority required".to_owned())?;
        Ok(ProviderRouteContext {
            instance_id,
            provider_kind,
            api_surface,
            credential_route,
            incarnation: self.incarnation,
            registry_generation: self.registry_generation,
            binding_generation: self.binding_generation,
            credential_generation: self.credential_generation,
            authority,
            origin: self.origin,
            model_partition: self.model_partition,
            pacing: self.pacing,
        })
    }
}

fn origin_from_url(base_url: &str) -> Option<String> {
    // Lightweight host extract without the `url` crate dependency.
    let rest = base_url
        .strip_prefix("https://")
        .or_else(|| base_url.strip_prefix("http://"))
        .unwrap_or(base_url);
    let hostport = rest.split('/').next().filter(|s| !s.is_empty())?;
    Some(hostport.to_owned())
}

fn default_surface_and_route(kind: RouteProviderKind) -> (RouteApiSurface, RouteCredentialRoute) {
    match kind {
        RouteProviderKind::Xai => (
            RouteApiSurface::OpenAiCompatibleSubset,
            RouteCredentialRoute::XaiSession,
        ),
        RouteProviderKind::OpenAi => (
            RouteApiSurface::OpenAiPlatform,
            RouteCredentialRoute::OpenAiPlatform,
        ),
        RouteProviderKind::OpenRouter => (
            RouteApiSurface::OpenRouterNative,
            RouteCredentialRoute::ApiKey,
        ),
        RouteProviderKind::Anthropic => (
            RouteApiSurface::AnthropicMessages,
            RouteCredentialRoute::ApiKey,
        ),
        RouteProviderKind::Zai | RouteProviderKind::OpenAiCompatible => (
            RouteApiSurface::OpenAiCompatibleSubset,
            RouteCredentialRoute::ApiKey,
        ),
        RouteProviderKind::Custom => (
            RouteApiSurface::OpenAiCompatibleSubset,
            RouteCredentialRoute::None,
        ),
    }
}

/// How a config update interacts with an existing route context.
#[derive(Debug, Clone)]
pub enum RouteContextUpdate {
    /// Re-derive a legacy host context from the new config (clear explicit).
    DeriveLegacy,
    /// Replace with an explicit validated context.
    Replace(ProviderRouteContext),
    /// Clear any explicit context (next turn re-derives).
    Clear,
}

impl fmt::Display for RouteAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{InferenceConfig, ProviderIdentity};

    fn cfg(base: &str, model: &str, identity: ProviderIdentity) -> InferenceConfig {
        InferenceConfig {
            base_url: base.into(),
            model: model.into(),
            provider_identity: identity,
            ..InferenceConfig::default()
        }
    }

    #[test]
    fn route_authority_wire_unknown_fails_closed() {
        assert!(RouteAuthority::from_wire("authoritative").is_some());
        assert!(RouteAuthority::from_wire("unverified").is_some());
        assert!(RouteAuthority::from_wire("host_fallback").is_some());
        assert!(RouteAuthority::from_wire("trusted").is_none());
        assert!(RouteAuthority::from_wire("").is_none());
    }

    #[test]
    fn legacy_from_config_uses_identity_then_host_fallback() {
        let openrouter = ProviderRouteContext::legacy_from_config(&cfg(
            "https://openrouter.ai/api/v1",
            "m",
            ProviderIdentity::OpenRouter,
        ));
        assert_eq!(openrouter.provider_kind(), RouteProviderKind::OpenRouter);
        assert!(openrouter.authority().is_non_authoritative());
        assert_ne!(openrouter.authority(), RouteAuthority::HostFallback);

        let custom = ProviderRouteContext::legacy_from_config(&cfg(
            "https://proxy.example/v1",
            "m",
            ProviderIdentity::Custom,
        ));
        assert_eq!(custom.authority(), RouteAuthority::HostFallback);
    }

    #[test]
    fn pacing_partition_includes_incarnation() {
        let a = ProviderRouteContext::builder()
            .instance_id("openrouter_work")
            .provider_kind(RouteProviderKind::OpenRouter)
            .api_surface(RouteApiSurface::OpenRouterNative)
            .credential_route(RouteCredentialRoute::ApiKey)
            .incarnation("123e4567-e89b-12d3-a456-426614174000")
            .authority(RouteAuthority::Authoritative)
            .build()
            .unwrap();
        let b = ProviderRouteContext::builder()
            .instance_id("openrouter_work")
            .provider_kind(RouteProviderKind::OpenRouter)
            .api_surface(RouteApiSurface::OpenRouterNative)
            .credential_route(RouteCredentialRoute::ApiKey)
            .incarnation("123e4567-e89b-12d3-a456-426614174001")
            .authority(RouteAuthority::Authoritative)
            .build()
            .unwrap();
        assert_ne!(a.pacing_partition_key(), b.pacing_partition_key());
    }

    #[test]
    fn sibling_accounts_have_distinct_pacing_partitions() {
        let a = ProviderRouteContext::builder()
            .instance_id("corp_a")
            .provider_kind(RouteProviderKind::OpenAiCompatible)
            .api_surface(RouteApiSurface::OpenAiCompatibleSubset)
            .credential_route(RouteCredentialRoute::ApiKey)
            .authority(RouteAuthority::Authoritative)
            .build()
            .unwrap();
        let b = ProviderRouteContext::builder()
            .instance_id("corp_b")
            .provider_kind(RouteProviderKind::OpenAiCompatible)
            .api_surface(RouteApiSurface::OpenAiCompatibleSubset)
            .credential_route(RouteCredentialRoute::ApiKey)
            .authority(RouteAuthority::Authoritative)
            .build()
            .unwrap();
        assert_ne!(a.pacing_partition_key(), b.pacing_partition_key());
    }

    #[test]
    fn serde_roundtrip_validates() {
        let ctx = ProviderRouteContext::builder()
            .instance_id("openai")
            .provider_kind(RouteProviderKind::OpenAi)
            .api_surface(RouteApiSurface::OpenAiPlatform)
            .credential_route(RouteCredentialRoute::ApiKey)
            .binding_generation(3)
            .authority(RouteAuthority::Authoritative)
            .build()
            .unwrap();
        let json = serde_json::to_string(&ctx).unwrap();
        let back: ProviderRouteContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back.binding_generation(), 3);
        assert_eq!(back.authority(), RouteAuthority::Authoritative);
    }

    #[test]
    fn serde_unknown_authority_fails() {
        let bad = r#"{"instance_id":"x","kind_label":"xai","credential_route":"xai_session","authority":"trusted"}"#;
        assert!(serde_json::from_str::<ProviderRouteContext>(bad).is_err());
    }
}
