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
    /// Derived from a pinned official OpenAPI baseline inventory.
    OfficialBaseline,
    /// Explicit intersection or native declaration inventory.
    InventoryDeclaration,
    /// Typed client binding table (later milestones).
    ClientBinding,
    /// Runtime probe (later milestones; never invented here).
    RuntimeProbe,
    /// Unknown / future evidence kinds deserialize here when tagged oddly.
    #[serde(other)]
    Other,
}

/// Provenance stamp for a single evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: EvidenceKind,
    /// Human-readable source label (URL, inventory path, or declaration id).
    pub source: String,
    /// ISO-8601 UTC timestamp when the evidence was recorded.
    pub timestamp_utc: String,
    /// Baseline document version when applicable (e.g. OpenAPI `info.version`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_version: Option<String>,
    /// Content SHA-256 of the pinned source blob when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,
}

/// High-level API family for operation grouping.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiFamily {
    ChatCompletions,
    Responses,
    /// Anthropic-compatible Messages surface (OpenRouter-native relative to OpenAI).
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
    /// Map a path template to a family; unknown paths become `Other`.
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

/// HTTP method for baseline operations.
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

/// Transport classification derived conservatively from OpenAPI shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    HttpJson,
    HttpSse,
    HttpMultipart,
    Websocket,
    #[default]
    Unknown,
}

impl Transport {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "http_json" => Self::HttpJson,
            "http_sse" => Self::HttpSse,
            "http_multipart" => Self::HttpMultipart,
            "websocket" | "ws" => Self::Websocket,
            _ => Self::Unknown,
        }
    }
}

/// Stable operation identity used across baselines and intersection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationIdentity {
    pub family: ApiFamily,
    /// Stable id (OpenAPI operationId or declared shared id).
    pub operation_id: String,
    pub method: HttpMethod,
    /// OpenAPI path template (e.g. `/chat/completions`).
    pub path: String,
    pub transport: Transport,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_types: Vec<String>,
}

impl OperationIdentity {
    /// Deterministic method+path key.
    pub fn method_path_key(&self) -> String {
        format!("{} {}", self.method.as_str(), self.path)
    }
}

/// Which claim surface a status applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimSurface {
    /// Completeness of a typed OpenAI client vs the OpenAI baseline.
    OpenaiClientCompleteness,
    /// Coverage of OpenRouter-native operations (not in OpenAI baseline).
    OpenrouterNativeCoverage,
    /// Capability advertised by a configured third-party provider.
    ConfiguredProviderCapability,
}

/// Binding of an operation into product surfaces (client library / CLI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BindingStatus {
    /// Typed binding exists and is covered by tests (later milestones).
    Implemented,
    /// Explicitly not wired yet — honest default for Change 4.
    #[default]
    NotImplemented,
    /// Unknown / not assessed.
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
    // Reject scheme-like absolute URLs smuggled as paths.
    if path.contains("://") {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_safety_rejects_traversal_and_urls() {
        assert!(path_is_safe("/chat/completions"));
        assert!(path_is_safe("/files/{file_id}"));
        assert!(!path_is_safe("chat/completions"));
        assert!(!path_is_safe("//evil"));
        assert!(!path_is_safe("/../etc/passwd"));
        assert!(!path_is_safe("https://evil.example/x"));
    }

    #[test]
    fn unknown_status_is_default() {
        assert_eq!(CompatibilityStatus::default(), CompatibilityStatus::Unknown);
    }

    #[test]
    fn binding_defaults_to_not_implemented() {
        assert_eq!(BindingStatus::default(), BindingStatus::NotImplemented);
    }

    #[test]
    fn family_from_path_covers_coding_agent_routes() {
        assert_eq!(
            ApiFamily::from_path("/chat/completions"),
            ApiFamily::ChatCompletions
        );
        assert_eq!(ApiFamily::from_path("/responses"), ApiFamily::Responses);
        assert_eq!(ApiFamily::from_path("/messages"), ApiFamily::Messages);
        assert!(matches!(
            ApiFamily::from_path("/alpha/experimental"),
            ApiFamily::Other(_)
        ));
    }
}
