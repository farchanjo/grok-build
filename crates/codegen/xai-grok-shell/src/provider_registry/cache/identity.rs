//! Secret-free identity material for provider cache envelopes.
//!
//! Credential binding IDs are random opaque UUIDs and are never derived from a
//! secret. Organization/project values are stored only as a route-affecting
//! fingerprint (SHA-256), never as raw strings.

use super::super::id::ProviderId;
use super::super::instance::{
    ApiSurface, CredentialRoute, IncarnationError, ProviderIncarnation, ProviderKind,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

/// SHA-256 hex length for org/project fingerprints.
pub const ORG_PROJECT_FINGERPRINT_HEX_LEN: usize = 64;

/// Random opaque credential-binding identifier. Never a key fingerprint, never
/// derived from secret material, and never used as a verification oracle.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CredentialBindingId(String);

impl CredentialBindingId {
    /// Validate a canonical UUID-form token (same grammar as incarnation).
    pub fn new(raw: impl AsRef<str>) -> Result<Self, IncarnationError> {
        let incarnation = ProviderIncarnation::new(raw)?;
        Ok(Self(incarnation.as_str().to_owned()))
    }

    /// Mint a fresh random binding id (UUID v4 textual form).
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for CredentialBindingId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CredentialBindingId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for CredentialBindingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Expected identity for reading or publishing an authoritative cache envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderCacheIdentity {
    pub instance_id: ProviderId,
    pub incarnation: ProviderIncarnation,
    pub kind: ProviderKind,
    pub api_surface: ApiSurface,
    pub credential_route: CredentialRoute,
    /// Normalized endpoint origin only (scheme + host [+ port]).
    pub endpoint_origin: String,
    /// Route-affecting organization/project fingerprint (lowercase SHA-256 hex),
    /// or empty when neither is configured.
    pub org_project_fingerprint: String,
    pub credential_binding_id: CredentialBindingId,
}

impl ProviderCacheIdentity {
    /// Construct an identity after validating the org/project fingerprint form.
    pub fn new(
        instance_id: ProviderId,
        incarnation: ProviderIncarnation,
        kind: ProviderKind,
        api_surface: ApiSurface,
        credential_route: CredentialRoute,
        endpoint_origin: impl Into<String>,
        org_project_fingerprint: impl Into<String>,
        credential_binding_id: CredentialBindingId,
    ) -> Result<Self, FingerprintError> {
        let org_project_fingerprint = org_project_fingerprint.into();
        validate_org_project_fingerprint(&org_project_fingerprint)?;
        Ok(Self {
            instance_id,
            incarnation,
            kind,
            api_surface,
            credential_route,
            endpoint_origin: endpoint_origin.into(),
            org_project_fingerprint,
            credential_binding_id,
        })
    }
}

/// Normalize a base URL to the route-affecting origin only.
pub fn normalize_endpoint_origin(base_url: &str) -> Result<String, OriginNormalizeError> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err(OriginNormalizeError::Empty);
    }
    let url = url::Url::parse(trimmed).map_err(|_| OriginNormalizeError::Invalid)?;
    match url.scheme() {
        "http" | "https" => {}
        _ => return Err(OriginNormalizeError::UnsupportedScheme),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(OriginNormalizeError::CredentialsPresent);
    }
    let host = url
        .host_str()
        .ok_or(OriginNormalizeError::MissingHost)?
        .to_ascii_lowercase();
    if host.contains('%') {
        return Err(OriginNormalizeError::Invalid);
    }
    match url.port() {
        Some(port) => Ok(format!("{}://{host}:{port}", url.scheme())),
        None => Ok(format!("{}://{host}", url.scheme())),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginNormalizeError {
    Empty,
    Invalid,
    UnsupportedScheme,
    CredentialsPresent,
    MissingHost,
}

impl std::fmt::Display for OriginNormalizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "endpoint origin is empty"),
            Self::Invalid => write!(f, "endpoint origin is not a valid absolute URL"),
            Self::UnsupportedScheme => write!(f, "endpoint origin scheme must be http or https"),
            Self::CredentialsPresent => {
                write!(f, "endpoint origin must not embed credentials")
            }
            Self::MissingHost => write!(f, "endpoint origin is missing a host"),
        }
    }
}

impl std::error::Error for OriginNormalizeError {}

/// Fingerprint organization/project for route identity without storing raw values.
///
/// Encoding is unambiguous: each field is length-prefixed (u64 LE) under a
/// versioned domain tag so embedded NUL bytes cannot collide across components.
pub fn org_project_fingerprint(organization: Option<&str>, project: Option<&str>) -> String {
    let org = organization
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let proj = project
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    if org.is_empty() && proj.is_empty() {
        return String::new();
    }
    let mut hasher = Sha256::new();
    hasher.update(b"provider-cache-org-project-v2\0");
    write_len_prefixed(&mut hasher, org.as_bytes());
    write_len_prefixed(&mut hasher, proj.as_bytes());
    hex_digest(hasher.finalize().as_slice())
}

fn write_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// Accept only empty or 64-char lowercase hex SHA-256 digests.
pub fn validate_org_project_fingerprint(fp: &str) -> Result<(), FingerprintError> {
    if fp.is_empty() {
        return Ok(());
    }
    if fp.len() != ORG_PROJECT_FINGERPRINT_HEX_LEN {
        return Err(FingerprintError::InvalidLength { len: fp.len() });
    }
    if !fp.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(FingerprintError::InvalidCharset);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FingerprintError {
    InvalidLength { len: usize },
    InvalidCharset,
}

impl std::fmt::Display for FingerprintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLength { len } => write!(
                f,
                "org/project fingerprint length {len} is not empty or {ORG_PROJECT_FINGERPRINT_HEX_LEN} hex chars"
            ),
            Self::InvalidCharset => {
                write!(f, "org/project fingerprint must be lowercase hex or empty")
            }
        }
    }
}

impl std::error::Error for FingerprintError {}

fn hex_digest(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_normalizes_host_port_and_lowercases() {
        assert_eq!(
            normalize_endpoint_origin("https://API.OpenAI.com/v1/models").unwrap(),
            "https://api.openai.com"
        );
        assert_eq!(
            normalize_endpoint_origin("http://127.0.0.1:8000/v1").unwrap(),
            "http://127.0.0.1:8000"
        );
    }

    #[test]
    fn origin_collapses_default_ports_and_strips_query_fragment() {
        assert_eq!(
            normalize_endpoint_origin("https://api.openai.com:443/v1?x=1#frag").unwrap(),
            "https://api.openai.com"
        );
        assert_eq!(
            normalize_endpoint_origin("http://example.com:80/path").unwrap(),
            "http://example.com"
        );
    }

    #[test]
    fn origin_keeps_ipv6_brackets_without_zone() {
        assert_eq!(
            normalize_endpoint_origin("http://[2001:db8::1]:8080/v1").unwrap(),
            "http://[2001:db8::1]:8080"
        );
    }

    #[test]
    fn origin_rejects_credentials_and_non_http() {
        assert!(matches!(
            normalize_endpoint_origin("https://user:pass@api.openai.com/v1"),
            Err(OriginNormalizeError::CredentialsPresent)
        ));
        assert!(matches!(
            normalize_endpoint_origin("https://user@api.openai.com/v1"),
            Err(OriginNormalizeError::CredentialsPresent)
        ));
        assert!(matches!(
            normalize_endpoint_origin("file:///tmp/x"),
            Err(OriginNormalizeError::UnsupportedScheme)
        ));
    }

    #[test]
    fn org_project_fingerprint_is_stable_secret_free_and_unambiguous() {
        let a = org_project_fingerprint(Some("org-a"), Some("proj-1"));
        let b = org_project_fingerprint(Some("org-a"), Some("proj-1"));
        let c = org_project_fingerprint(Some("org-b"), Some("proj-1"));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(!a.contains("org-a"));
        assert_eq!(a.len(), ORG_PROJECT_FINGERPRINT_HEX_LEN);
        assert_eq!(org_project_fingerprint(None, None), "");
        let left = org_project_fingerprint(Some("a\0b"), Some("c"));
        let right = org_project_fingerprint(Some("a"), Some("b\0c"));
        assert_ne!(left, right);
        validate_org_project_fingerprint(&left).unwrap();
        assert!(validate_org_project_fingerprint("not-hex").is_err());
        assert!(validate_org_project_fingerprint(&"A".repeat(64)).is_err());
        assert!(
            ProviderCacheIdentity::new(
                ProviderId::new("openai").unwrap(),
                ProviderIncarnation::new("11111111-1111-1111-1111-111111111111").unwrap(),
                ProviderKind::OpenAi,
                ApiSurface::OpenAiPlatform,
                CredentialRoute::ApiKey,
                "https://api.openai.com",
                "raw-org-name",
                CredentialBindingId::generate(),
            )
            .is_err()
        );
    }

    #[test]
    fn binding_id_is_random_not_derived() {
        let a = CredentialBindingId::generate();
        let b = CredentialBindingId::generate();
        assert_ne!(a.as_str(), b.as_str());
        assert!(CredentialBindingId::new(a.as_str()).is_ok());
        assert!(CredentialBindingId::new("not-a-uuid").is_err());
    }
}
