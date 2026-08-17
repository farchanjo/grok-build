//! Provider metadata snapshot and namespaced model ID helpers.

use super::id::{BuiltInProviderId, ProviderId, ProviderRef};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Explicit capability overrides stored on a provider.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_completions: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responses: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embeddings: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batches: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fine_tuning: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin: Option<bool>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, bool>,
}

/// Capability discovery mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityMode {
    #[default]
    Auto,
    Manual,
    Off,
}

/// Auth scheme for OpenAI-compatible providers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthScheme {
    #[default]
    Bearer,
    None,
    CustomHeader,
}

/// Rich metadata for one registry entry (built-in or configured).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderMetadata {
    pub id: ProviderId,
    pub provider_ref: ProviderRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// `openai_compatible`, `openai`, `openrouter`, `xai`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_base_url: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_backend: Option<String>,
    #[serde(default)]
    pub auth_scheme: ProviderAuthScheme,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_env_key: Option<String>,
    #[serde(default)]
    pub catalog_enabled: bool,
    #[serde(default)]
    pub capability_mode: CapabilityMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_ttl_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_timeout_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default)]
    pub extra_headers: IndexMap<String, String>,
    #[serde(default)]
    pub capabilities: CapabilityOverrides,
}

fn default_enabled() -> bool {
    true
}

impl ProviderMetadata {
    pub fn display_label(&self) -> String {
        self.display_name
            .clone()
            .unwrap_or_else(|| self.id.as_str().to_owned())
    }

    pub fn is_openai_compatible_family(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "openai_compatible" | "custom" | "openai" | "openrouter" | "zai"
        )
    }
}

/// Snapshot of all known providers after config merge.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProviderRegistrySnapshot {
    pub providers: IndexMap<String, ProviderMetadata>,
    pub warnings: Vec<String>,
}

impl ProviderRegistrySnapshot {
    pub fn get(&self, id: &str) -> Option<&ProviderMetadata> {
        self.providers.get(id)
    }

    pub fn enabled_providers(&self) -> impl Iterator<Item = &ProviderMetadata> {
        self.providers.values().filter(|p| p.enabled)
    }

    pub fn insert_built_in(
        &mut self,
        built_in: BuiltInProviderId,
        base_url: Option<String>,
        kind: &str,
    ) {
        let id = ProviderId::from_built_in(built_in);
        self.providers.insert(
            id.as_str().to_owned(),
            ProviderMetadata {
                id: id.clone(),
                provider_ref: ProviderRef::BuiltIn(built_in),
                display_name: Some(built_in.display_name().to_owned()),
                kind: kind.to_owned(),
                base_url,
                admin_base_url: None,
                enabled: true,
                default_backend: None,
                auth_scheme: ProviderAuthScheme::Bearer,
                env_key: None,
                admin_env_key: None,
                catalog_enabled: true,
                capability_mode: CapabilityMode::Auto,
                catalog_ttl_secs: None,
                request_timeout_secs: None,
                organization: None,
                project: None,
                extra_headers: IndexMap::new(),
                capabilities: CapabilityOverrides::default(),
            },
        );
    }
}

/// Namespaced catalog model id: `{provider_id}:{upstream_slug}`.
pub fn namespaced_model_id(provider_id: &ProviderId, upstream_slug: &str) -> String {
    format!("{}:{upstream_slug}", provider_id.as_str())
}

/// Split a namespaced model id. Returns `(provider_id, slug)` when namespaced.
pub fn parse_namespaced_model_id(model_id: &str) -> Option<(ProviderId, String)> {
    let (left, right) = model_id.split_once(':')?;
    if right.is_empty() {
        return None;
    }
    let id = ProviderId::new(left).ok()?;
    Some((id, right.to_owned()))
}

/// Resolve a legacy un-namespaced alias when exactly one provider advertises it.
///
/// Returns `None` when zero or multiple providers claim the same upstream slug
/// (ambiguous).
pub fn resolve_legacy_model_alias(
    slug: &str,
    catalog_by_provider: &IndexMap<String, Vec<String>>,
) -> Option<String> {
    let mut matches: Vec<String> = Vec::new();
    for (provider_id, models) in catalog_by_provider {
        if models
            .iter()
            .any(|m| m == slug || m.ends_with(&format!(":{slug}")))
        {
            let pid = ProviderId::new(provider_id).ok()?;
            matches.push(namespaced_model_id(&pid, slug));
        }
    }
    if matches.len() == 1 {
        matches.pop()
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderLifecycleError {
    InvalidId(String),
    InvalidUrl(String),
    DuplicateId(String),
    NotFound(String),
    ReservedId(String),
    InvalidHeader(String),
    Validation(String),
}

impl std::fmt::Display for ProviderLifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidId(m) => write!(f, "invalid provider id: {m}"),
            Self::InvalidUrl(m) => write!(f, "invalid base URL: {m}"),
            Self::DuplicateId(id) => write!(f, "duplicate provider id `{id}`"),
            Self::NotFound(id) => write!(f, "provider `{id}` not found"),
            Self::ReservedId(id) => write!(f, "provider id `{id}` is reserved"),
            Self::InvalidHeader(m) => write!(f, "invalid header: {m}"),
            Self::Validation(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for ProviderLifecycleError {}

/// Validate a base URL string without requiring the inference crate.
pub fn validate_http_base_url(raw: &str) -> Result<(), ProviderLifecycleError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ProviderLifecycleError::InvalidUrl("empty".into()));
    }
    let url = reqwest::Url::parse(trimmed)
        .map_err(|e| ProviderLifecycleError::InvalidUrl(e.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ProviderLifecycleError::InvalidUrl(format!(
            "unsupported scheme {}",
            url.scheme()
        )));
    }
    if url.username() != "" || url.password().is_some() {
        return Err(ProviderLifecycleError::InvalidUrl(
            "must not embed credentials".into(),
        ));
    }
    if url.host_str().is_none() {
        return Err(ProviderLifecycleError::InvalidUrl("missing host".into()));
    }
    Ok(())
}

/// Validate extra headers for management save and resolve.
///
/// Rejects case-insensitive restricted names so typed fields own them and
/// free-form extras cannot collide with auth or org/project identity:
/// - `Authorization` / `Cookie` / `Proxy-Authorization`
/// - `OpenAI-Organization` / `OpenAI-Project` (typed provider fields own these)
/// - `Content-Type` / `Accept` (transport owns negotiation)
/// - `x-grok-*` first-party headers
/// - values containing CR/LF/control characters
pub fn validate_extra_headers(
    headers: &IndexMap<String, String>,
) -> Result<(), ProviderLifecycleError> {
    for (k, v) in headers {
        let lower = k.to_ascii_lowercase();
        if lower == "authorization"
            || lower == "cookie"
            || lower == "proxy-authorization"
            || lower == "openai-organization"
            || lower == "openai-project"
            || lower == "content-type"
            || lower == "accept"
        {
            return Err(ProviderLifecycleError::InvalidHeader(format!(
                "restricted header `{k}`"
            )));
        }
        if lower.starts_with("x-grok-") {
            return Err(ProviderLifecycleError::InvalidHeader(format!(
                "first-party header `{k}` not allowed on custom providers"
            )));
        }
        if v.chars().any(|c| c == '\r' || c == '\n' || c.is_control()) {
            return Err(ProviderLifecycleError::InvalidHeader(format!(
                "header `{k}` contains control characters"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespaced_ids_round_trip() {
        let id = ProviderId::new("local_vllm").unwrap();
        let ns = namespaced_model_id(&id, "deepseek-coder");
        assert_eq!(ns, "local_vllm:deepseek-coder");
        let (p, slug) = parse_namespaced_model_id(&ns).unwrap();
        assert_eq!(p.as_str(), "local_vllm");
        assert_eq!(slug, "deepseek-coder");
    }

    #[test]
    fn legacy_alias_unambiguous_only() {
        let mut catalog = IndexMap::new();
        catalog.insert("a".into(), vec!["m1".into()]);
        catalog.insert("b".into(), vec!["m2".into()]);
        assert_eq!(
            resolve_legacy_model_alias("m1", &catalog).as_deref(),
            Some("a:m1")
        );
        catalog.insert("c".into(), vec!["m1".into()]);
        assert!(resolve_legacy_model_alias("m1", &catalog).is_none());
    }

    #[test]
    fn validates_urls_and_headers() {
        assert!(validate_http_base_url("https://api.z.ai/api/paas/v4").is_ok());
        assert!(validate_http_base_url("https://user:pass@x/").is_err());
        let mut h = IndexMap::new();
        h.insert("Authorization".into(), "Bearer x".into());
        assert!(validate_extra_headers(&h).is_err());
    }

    #[test]
    fn extra_headers_reject_org_project_and_content_negotiation() {
        for name in [
            "OpenAI-Organization",
            "openai-project",
            "Content-Type",
            "ACCEPT",
        ] {
            let mut h = IndexMap::new();
            h.insert(name.into(), "x".into());
            assert!(
                validate_extra_headers(&h).is_err(),
                "expected reject for {name}"
            );
        }
        let mut ok = IndexMap::new();
        ok.insert("X-Custom-Lab".into(), "1".into());
        assert!(validate_extra_headers(&ok).is_ok());
    }
}
