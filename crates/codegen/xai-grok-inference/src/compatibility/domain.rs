//! Compatibility domain model for OpenAI / OpenRouter baselines.
//!
//! Types are owned and extensible for later registry/CLI milestones.
//! Unknown or additive values fail safely at the claim layer.

use serde::{Deserialize, Serialize};

/// Tri-state compatibility verdict for a named claim surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    /// Explicitly present and accepted for the claim surface.
    Supported,
    /// Explicitly out of scope for the claim surface.
    Unsupported,
    /// No evidence yet — fail closed for capability claims.
    #[default]
    Unknown,
}

/// How a claim was evidenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    OfficialBaseline,
    InventoryDeclaration,
    ClientBinding,
    RuntimeProbe,
    #[serde(other)]
    Other,
}

/// Provenance stamp for a single evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub source: String,
    pub timestamp_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,
}

/// High-level API family for operation grouping.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiFamily {
    ChatCompletions,
    Responses,
    Messages,
    Embeddings,
    Models,
    Audio,
    Files,
    Videos,
    Images,
    Moderations,
    FineTuning,
    Batches,
    Assistants,
    VectorStores,
    Other(String),
}

impl ApiFamily {
    pub fn from_path(path: &str) -> Self {
        let p = path.trim();
        if p.starts_with("/chat/completions") {
            Self::ChatCompletions
        } else if p.starts_with("/responses") {
            Self::Responses
        } else if p.starts_with("/messages") {
            Self::Messages
        } else if p.starts_with("/embeddings") {
            Self::Embeddings
        } else if p.starts_with("/models") {
            Self::Models
        } else if p.starts_with("/audio/") {
            Self::Audio
        } else if p.starts_with("/files") {
            Self::Files
        } else if p.starts_with("/videos") {
            Self::Videos
        } else if p.starts_with("/images") {
            Self::Images
        } else if p.starts_with("/moderations") {
            Self::Moderations
        } else if p.starts_with("/fine_tuning") || p.starts_with("/fine-tuning") {
            Self::FineTuning
        } else if p.starts_with("/batches") {
            Self::Batches
        } else if p.starts_with("/assistants") {
            Self::Assistants
        } else if p.starts_with("/vector_stores") {
            Self::VectorStores
        } else {
            Self::Other(p.to_owned())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Put,
    Post,
    Delete,
    Options,
    Head,
    Patch,
    Trace,
}

impl HttpMethod {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "GET" => Some(Self::Get),
            "PUT" => Some(Self::Put),
            "POST" => Some(Self::Post),
            "DELETE" => Some(Self::Delete),
            "OPTIONS" => Some(Self::Options),
            "HEAD" => Some(Self::Head),
            "PATCH" => Some(Self::Patch),
            "TRACE" => Some(Self::Trace),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Put => "PUT",
            Self::Post => "POST",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
            Self::Head => "HEAD",
            Self::Patch => "PATCH",
            Self::Trace => "TRACE",
        }
    }
}

/// Transport classification from official OpenAPI shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    HttpJson,
    HttpSse,
    HttpMultipart,
    HttpBinary,
    Websocket,
    #[default]
    Unknown,
}

impl Transport {
    pub const ALL_LABELS: &'static [&'static str] = &[
        "http_json",
        "http_sse",
        "http_multipart",
        "http_binary",
        "websocket",
        "unknown",
    ];

    /// Parse a strict inventory label. Returns `None` for invalid labels
    /// (callers that require validation should treat `None` as an error).
    pub fn parse_strict(raw: &str) -> Option<Self> {
        match raw.trim() {
            "http_json" => Some(Self::HttpJson),
            "http_sse" => Some(Self::HttpSse),
            "http_multipart" => Some(Self::HttpMultipart),
            "http_binary" => Some(Self::HttpBinary),
            "websocket" => Some(Self::Websocket),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::HttpJson => "http_json",
            Self::HttpSse => "http_sse",
            Self::HttpMultipart => "http_multipart",
            Self::HttpBinary => "http_binary",
            Self::Websocket => "websocket",
            Self::Unknown => "unknown",
        }
    }
}

/// Stable operation identity used across baselines and intersection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationIdentity {
    pub family: ApiFamily,
    pub operation_id: String,
    pub method: HttpMethod,
    pub path: String,
    /// Multi-label transport set (JSON + SSE for stream-flag ops, etc.).
    pub transports: Vec<Transport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub request_content_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub response_content_types: Vec<String>,
}

impl OperationIdentity {
    pub fn method_path_key(&self) -> String {
        format!("{} {}", self.method.as_str(), self.path)
    }
}

/// Which claim surface a status applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimSurface {
    /// Operation exists in the official OpenAI baseline inventory.
    OpenaiBaselinePresence,
    /// Typed OpenAI client binding completeness (Change 9+).
    OpenaiClientCompleteness,
    /// CLI binding coverage (later milestones).
    CliCoverage,
    /// OpenRouter-native exclusive operations (path not in OpenAI baseline).
    OpenrouterNativeCoverage,
    /// Configured third-party provider capability.
    ConfiguredProviderCapability,
}

/// Binding of an operation into product surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BindingStatus {
    Implemented,
    #[default]
    NotImplemented,
    Unknown,
}

impl BindingStatus {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "implemented" => Self::Implemented,
            "not_implemented" => Self::NotImplemented,
            _ => Self::Unknown,
        }
    }
}

/// One auditable claim about an operation on a claim surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationClaim {
    pub identity: OperationIdentity,
    pub surface: ClaimSurface,
    pub status: CompatibilityStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub client_binding: BindingStatus,
    #[serde(default)]
    pub cli_binding: BindingStatus,
}

/// Validate an OpenAPI-style path template for inventory safety.
pub fn path_is_safe(path: &str) -> bool {
    if path.is_empty() || !path.starts_with('/') {
        return false;
    }
    if path.starts_with("//") || path.contains("..") {
        return false;
    }
    if path.contains('\0') || path.contains('\n') || path.contains('\r') {
        return false;
    }
    if path.contains("://") {
        return false;
    }
    true
}

/// Basic media-type syntax check (type/subtype or type/*).
pub fn media_type_is_valid(mt: &str) -> bool {
    let s = mt.trim();
    if s.is_empty() || s.len() > 256 {
        return false;
    }
    if s == "*/*" {
        return true;
    }
    let main = s.split(';').next().unwrap_or("").trim();
    let Some((t, st)) = main.split_once('/') else {
        return false;
    };
    if t.is_empty() || st.is_empty() {
        return false;
    }
    let ok = |p: &str| {
        p.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '!' | '#' | '$' | '&' | '-' | '^' | '_' | '+' | '.' | '*')
        })
    };
    ok(t) && ok(st)
}

/// RFC3339-ish UTC timestamp check (`YYYY-MM-DDTHH:MM:SSZ`).
pub fn timestamp_is_rfc3339_utc(ts: &str) -> bool {
    let b = ts.as_bytes();
    if b.len() != 20 || b[19] != b'Z' {
        return false;
    }
    // 2026-07-25T16:25:32Z
    let digits = |i: usize| b[i].is_ascii_digit();
    digits(0)
        && digits(1)
        && digits(2)
        && digits(3)
        && b[4] == b'-'
        && digits(5)
        && digits(6)
        && b[7] == b'-'
        && digits(8)
        && digits(9)
        && b[10] == b'T'
        && digits(11)
        && digits(12)
        && b[13] == b':'
        && digits(14)
        && digits(15)
        && b[16] == b':'
        && digits(17)
        && digits(18)
}

/// Reject Supported status paired with unimplemented bindings on client/CLI surfaces.
pub fn claim_is_consistent(claim: &OperationClaim) -> Result<(), String> {
    match claim.surface {
        ClaimSurface::OpenaiClientCompleteness | ClaimSurface::CliCoverage => {
            if claim.status == CompatibilityStatus::Supported
                && (claim.client_binding == BindingStatus::NotImplemented
                    || claim.cli_binding == BindingStatus::NotImplemented)
            {
                return Err(
                    "Supported client/CLI completeness cannot pair with NotImplemented bindings"
                        .into(),
                );
            }
        }
        ClaimSurface::OpenaiBaselinePresence
        | ClaimSurface::OpenrouterNativeCoverage
        | ClaimSurface::ConfiguredProviderCapability => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_safety_rejects_traversal_and_urls() {
        assert!(path_is_safe("/chat/completions"));
        assert!(!path_is_safe("chat/completions"));
        assert!(!path_is_safe("//evil"));
        assert!(!path_is_safe("/../etc/passwd"));
        assert!(!path_is_safe("https://evil.example/x"));
    }

    #[test]
    fn media_type_validation() {
        assert!(media_type_is_valid("application/json"));
        assert!(media_type_is_valid("audio/*"));
        assert!(media_type_is_valid("text/event-stream"));
        assert!(!media_type_is_valid(""));
        assert!(!media_type_is_valid("no-slash"));
        assert!(!media_type_is_valid("application/"));
    }

    #[test]
    fn timestamp_validation() {
        assert!(timestamp_is_rfc3339_utc("2026-07-25T16:25:32Z"));
        assert!(!timestamp_is_rfc3339_utc("2026-07-25T17:00:00+00:00"));
        assert!(!timestamp_is_rfc3339_utc("not-a-time"));
    }

    #[test]
    fn transport_strict_rejects_unknown_labels() {
        assert!(Transport::parse_strict("http_json").is_some());
        assert!(Transport::parse_strict("http_binary").is_some());
        assert!(Transport::parse_strict("application/json").is_none());
        assert!(Transport::parse_strict("sse").is_none());
    }

    #[test]
    fn supported_plus_not_implemented_is_inconsistent_for_client_surface() {
        let claim = OperationClaim {
            identity: OperationIdentity {
                family: ApiFamily::ChatCompletions,
                operation_id: "x".into(),
                method: HttpMethod::Post,
                path: "/chat/completions".into(),
                transports: vec![Transport::HttpJson],
                request_content_types: vec![],
                response_content_types: vec![],
            },
            surface: ClaimSurface::OpenaiClientCompleteness,
            status: CompatibilityStatus::Supported,
            evidence: vec![],
            client_binding: BindingStatus::NotImplemented,
            cli_binding: BindingStatus::NotImplemented,
        };
        assert!(claim_is_consistent(&claim).is_err());
    }

    #[test]
    fn baseline_presence_supported_with_not_implemented_bindings_is_ok() {
        let claim = OperationClaim {
            identity: OperationIdentity {
                family: ApiFamily::ChatCompletions,
                operation_id: "x".into(),
                method: HttpMethod::Post,
                path: "/chat/completions".into(),
                transports: vec![Transport::HttpJson],
                request_content_types: vec![],
                response_content_types: vec![],
            },
            surface: ClaimSurface::OpenaiBaselinePresence,
            status: CompatibilityStatus::Supported,
            evidence: vec![],
            client_binding: BindingStatus::NotImplemented,
            cli_binding: BindingStatus::NotImplemented,
        };
        assert!(claim_is_consistent(&claim).is_ok());
    }
}
