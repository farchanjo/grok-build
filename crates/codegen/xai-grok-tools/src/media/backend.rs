//! Inference-free media-understanding request/result/error types and the
//! [`MediaUnderstandingBackend`] seam.
//!
//! The backend trait deliberately exposes no store, cache, ledger, inference,
//! consent, or route-registry internals (plan section 4.1). The shell-owned
//! `ShellMediaUnderstandingBackend` (PR 6) implements it; the session context
//! carries `Option<Arc<dyn MediaUnderstandingBackend>>` as a resource.

use crate::media::domain::{
    MediaCategory, MediaCategoryStrategy, MediaDetailLevel, MediaRouteMetadata, MediaSource,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A resolved media-understanding request handed to a backend.
///
/// Content-only: `MediaSource` cannot express URLs, so the initial
/// implementation has no SSRF surface by construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaUnderstandingRequest {
    /// Media items to analyze, in request order.
    pub media: Vec<MediaSource>,
    /// Category hint. `MediaCategory::Auto` defers to backend sniffing.
    pub category: MediaCategory,
    /// Optional user instruction that scopes the analysis. Bound at the tool
    /// boundary; never treated as trusted instructions by the backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    /// Requested detail level.
    #[serde(default)]
    pub detail: MediaDetailLevel,
    /// Optional semantic focus areas (for example `text`, `objects`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus: Vec<String>,
}

/// Structured semantics for one analyzed media item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaSemantics {
    /// The source this entry describes.
    pub source: MediaSource,
    /// Concrete category the backend resolved for this item.
    pub category: MediaCategory,
    /// Model-generated semantic text. Untrusted provenance is always labeled.
    pub text: String,
    /// Scrubbed provenance: provider/model/strategy, no credentials.
    pub provenance: MediaProvenance,
}

/// Non-secret provenance for a produced semantic result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaProvenance {
    /// Provider identity (for example `"xai"`, `"openrouter"`).
    pub provider: String,
    /// Concrete model ID that produced the semantics.
    pub model: String,
    /// Transport/strategy used.
    pub strategy: MediaCategoryStrategy,
}

/// Non-secret summary of one delegate attempt.
///
/// Never carries bytes, prompts, instruction text, tokens, or unsanitized
/// provider errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAttemptSummary {
    pub provider: String,
    pub model: String,
    /// Outcome label (for example `"success"`, `"skipped"`, `"failed"`).
    pub outcome: String,
    /// Optional non-secret reason for the outcome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Result of a media-understanding request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaUnderstandingResult {
    /// One entry per input media item, in request order.
    pub results: Vec<MediaSemantics>,
    /// Aggregate attempt summary across all routes tried.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attempts: Vec<MediaAttemptSummary>,
}

/// Terminal error produced by a [`MediaUnderstandingBackend`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum MediaUnderstandingError {
    /// No configured backend or no eligible route for the request.
    #[error("media understanding is not available: {0}")]
    Unavailable(String),
    /// The request failed input validation.
    #[error("invalid media input: {0}")]
    InvalidInput(String),
    /// Preprocessing (normalization, frame extraction, transcription) failed.
    #[error("media preprocessing failed: {0}")]
    PreprocessFailed(String),
    /// Every configured route was exhausted without a valid result.
    #[error("all configured routes were exhausted: {0}")]
    AllRoutesExhausted(String),
    /// A delegate returned a response that failed strict validation.
    #[error("delegate returned an invalid response: {0}")]
    InvalidDelegateResponse(String),
    /// The request was cancelled.
    #[error("request was cancelled")]
    Cancelled,
}

/// Availability snapshot a backend exposes for tool-listing and capability
/// gating (plan section 4.2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBackendAvailability {
    /// Whether media understanding is enabled for this session at all.
    #[serde(default)]
    pub enabled: bool,
    /// Concrete categories with at least one eligible route.
    #[serde(default)]
    pub supported_categories: Vec<MediaCategory>,
    /// Lightweight per-route metadata in configured order.
    #[serde(default)]
    pub routes: Vec<MediaRouteMetadata>,
}

impl MediaBackendAvailability {
    /// Whether the backend can serve at least one concrete category.
    ///
    /// This is the tool-listing gate: a disabled or unconfigured backend must
    /// not expose a dead `analyze_media` tool.
    pub fn has_eligible_route(&self) -> bool {
        self.enabled
            && self
                .supported_categories
                .iter()
                .any(|category| !matches!(category, MediaCategory::Auto))
    }
}

/// The inference-free seam between tools and the shell-owned media backend.
///
/// The concrete `ShellMediaUnderstandingBackend` (PR 6) owns all mutable and
/// inference-bearing state. This trait intentionally stays tiny and free of
/// concrete store, cache, ledger, inference, consent, and route-registry
/// internals so `xai-grok-tools` never depends on inference.
#[async_trait]
pub trait MediaUnderstandingBackend: Send + Sync + 'static {
    /// Analyze the requested media and return structured semantics.
    async fn analyze(
        &self,
        request: MediaUnderstandingRequest,
    ) -> Result<MediaUnderstandingResult, MediaUnderstandingError>;

    /// Current availability snapshot for this backend.
    fn availability(&self) -> MediaBackendAvailability;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::domain::{MediaCapabilities, MediaTransportCapabilities};
    use std::sync::Arc;

    #[test]
    fn request_round_trips_with_defaults() {
        let request = MediaUnderstandingRequest {
            media: vec![MediaSource::Path {
                path: "assets/photo.jpg".to_string(),
            }],
            category: MediaCategory::Image,
            instruction: None,
            detail: MediaDetailLevel::default(),
            focus: vec![],
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["media"][0]["kind"], "path");
        assert_eq!(json["category"], "image");
        assert!(json.get("instruction").is_none(), "None omitted");
        assert!(json.get("focus").is_none(), "empty focus omitted");
        let back: MediaUnderstandingRequest = serde_json::from_value(json).unwrap();
        assert_eq!(back, request);
    }

    #[test]
    fn request_round_trips_full() {
        let request = MediaUnderstandingRequest {
            media: vec![
                MediaSource::Path {
                    path: "a.png".to_string(),
                },
                MediaSource::ArtifactRef {
                    blob_hash: "deadbeef".to_string(),
                },
            ],
            category: MediaCategory::Auto,
            instruction: Some("Describe the visible error message".to_string()),
            detail: MediaDetailLevel::High,
            focus: vec!["text".to_string(), "objects".to_string()],
        };
        let back: MediaUnderstandingRequest =
            serde_json::from_value(serde_json::to_value(&request).unwrap()).unwrap();
        assert_eq!(back, request);
    }

    #[test]
    fn result_round_trips() {
        let result = MediaUnderstandingResult {
            results: vec![MediaSemantics {
                source: MediaSource::Path {
                    path: "a.png".to_string(),
                },
                category: MediaCategory::Image,
                text: "A terminal window with a syntax error".to_string(),
                provenance: MediaProvenance {
                    provider: "xai".to_string(),
                    model: "grok-4.5".to_string(),
                    strategy: MediaCategoryStrategy::Native,
                },
            }],
            attempts: vec![MediaAttemptSummary {
                provider: "xai".to_string(),
                model: "grok-4.5".to_string(),
                outcome: "success".to_string(),
                reason: None,
            }],
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["results"][0]["provenance"]["model"], "grok-4.5");
        let back: MediaUnderstandingResult = serde_json::from_value(json).unwrap();
        assert_eq!(back, result);
    }

    #[test]
    fn error_display_and_round_trip() {
        let error = MediaUnderstandingError::Unavailable("no backend injected".to_string());
        assert_eq!(
            error.to_string(),
            "media understanding is not available: no backend injected"
        );
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(json["Unavailable"], "no backend injected");
        let back: MediaUnderstandingError = serde_json::from_value(json).unwrap();
        assert_eq!(back, error);

        assert_eq!(
            MediaUnderstandingError::Cancelled.to_string(),
            "request was cancelled"
        );
    }

    #[test]
    fn availability_round_trips() {
        let availability = MediaBackendAvailability {
            enabled: true,
            supported_categories: vec![MediaCategory::Image],
            routes: vec![MediaRouteMetadata {
                model_id: "grok-4.5".to_string(),
                strategy: MediaCategoryStrategy::Native,
                unresolved: false,
                eligible: true,
            }],
        };
        let back: MediaBackendAvailability =
            serde_json::from_value(serde_json::to_value(&availability).unwrap()).unwrap();
        assert_eq!(back, availability);
    }

    #[test]
    fn availability_has_eligible_route_gate() {
        let disabled = MediaBackendAvailability {
            enabled: false,
            ..Default::default()
        };
        assert!(!disabled.has_eligible_route());

        let empty = MediaBackendAvailability {
            enabled: true,
            supported_categories: vec![],
            ..Default::default()
        };
        assert!(!empty.has_eligible_route());

        let auto_only = MediaBackendAvailability {
            enabled: true,
            supported_categories: vec![MediaCategory::Auto],
            ..Default::default()
        };
        assert!(!auto_only.has_eligible_route());

        let ready = MediaBackendAvailability {
            enabled: true,
            supported_categories: vec![MediaCategory::Image, MediaCategory::Video],
            ..Default::default()
        };
        assert!(ready.has_eligible_route());
    }

    /// The backend trait is object-safe and stays free of inference types.
    /// This compiles `Arc<dyn MediaUnderstandingBackend>` usage and confirms
    /// the resource-injection shape used by `SessionContext`.
    #[test]
    fn backend_is_object_safe_arc_compatible() {
        struct StubBackend;
        #[async_trait]
        impl MediaUnderstandingBackend for StubBackend {
            async fn analyze(
                &self,
                request: MediaUnderstandingRequest,
            ) -> Result<MediaUnderstandingResult, MediaUnderstandingError> {
                Ok(MediaUnderstandingResult {
                    results: request
                        .media
                        .into_iter()
                        .map(|source| MediaSemantics {
                            source,
                            category: MediaCategory::Image,
                            text: "stub semantics".to_string(),
                            provenance: MediaProvenance {
                                provider: "stub".to_string(),
                                model: "stub-model".to_string(),
                                strategy: MediaCategoryStrategy::Native,
                            },
                        })
                        .collect(),
                    attempts: vec![],
                })
            }

            fn availability(&self) -> MediaBackendAvailability {
                MediaBackendAvailability {
                    enabled: true,
                    supported_categories: vec![MediaCategory::Image],
                    routes: vec![],
                }
            }
        }
        let backend: Arc<dyn MediaUnderstandingBackend> = Arc::new(StubBackend);
        assert!(backend.availability().has_eligible_route());
    }

    /// Domain metadata types referenced from backend types stay round-trippable
    /// and inference-free by construction (compile-time check only).
    #[allow(dead_code)]
    fn _domain_metadata_smoke() -> (MediaCapabilities, MediaTransportCapabilities) {
        (
            MediaCapabilities::default(),
            MediaTransportCapabilities::default(),
        )
    }
}
