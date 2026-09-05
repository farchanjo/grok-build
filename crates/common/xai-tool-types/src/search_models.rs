//! Wire types for the out-of-tree `search_models` tool (Archanjo pack).
//!
//! Lives in the shared leaf so both `xai-grok-tools` (which carries the
//! `ToolOutput::SearchModels` payload) and `archanjo` (which implements the
//! tool) reference one definition — core must never depend on the pack.
//! The tool *input* type stays in the pack (it is the tool's `Args`, and the
//! pack owns the `From<SearchModelsInput> for ToolInput` conversion).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One catalog hit returned to the model.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SearchModelsHit {
    /// Human-readable display name (label).
    pub name: String,
    /// Catalog key for `spawn_subagent` `model=` (e.g. `openrouter:z-ai/glm-5.2`).
    /// Always the **canonical selection id**, never a bare upstream wire slug
    /// when siblings exist.
    pub slug: String,
    /// Provider id/kind string (e.g. `openrouter`, `xai`).
    pub provider: String,
    /// Secret-free provider instance id that owns this selection
    /// (e.g. `openai`, `openai_work`, `openrouter`). Distinguishes sibling
    /// accounts that share an upstream wire slug.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_instance_id: Option<String>,
    /// Provider kind label (`openai`, `openrouter`, `xai`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_kind: Option<String>,
    /// Exact upstream wire model id (never normalized). Distinct from `slug`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model_id: Option<String>,
    /// Catalog/copy description when advertised.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the upstream model advertises tool/function-calling support.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tools: Option<bool>,
    /// Explicitly advertised media-input capabilities. Tri-state: `true`/`false`,
    /// or `None` when the catalog is silent. Unknown is never treated as supported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_image_input: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_audio_input: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_video_input: Option<bool>,
    /// File/document input advertised by the catalog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_file_input: Option<bool>,
    /// `Some(false)` means the model is not a text agent (image/audio-only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_has_text: Option<bool>,
    /// Context window when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// Request budget (user/model override) when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    /// Routed completion-token capability ceiling when known (e.g. OpenRouter
    /// `top_provider.max_completion_tokens`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_ceiling: Option<u32>,
    /// Zero-data-retention capability when advertised.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_zdr: Option<bool>,
    /// Native response JSON schema alongside tools when advertised.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_native_schema: Option<bool>,
    /// Strict tool definitions support when advertised.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_strict_tools: Option<bool>,
    /// Reasoning-effort support flag. `None` means unknown — treat as unsupported
    /// for UI purposes, but an explicit effort is still honored on the wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_reasoning_effort: Option<bool>,
    /// Canonical reasoning-effort names the model advertises (e.g. `low`, `high`,
    /// `max`). Empty when unknown.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasoning_efforts: Vec<String>,
    /// Whether this slug passes the same gate as `Task.model` validation.
    pub task_eligible: bool,
    /// Explicit spawn hint: `spawn_subagent model="<slug>"`.
    pub call: String,
    /// BM25 / exact-match score for ranking diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
}

/// Structured result payload (also rendered as prompt text).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchModelsResult {
    pub results: Vec<SearchModelsHit>,
    /// True when the result set was truncated by `limit`.
    pub truncated: bool,
    /// Optional note (empty query summary, missing backend, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl SearchModelsHit {
    pub fn with_call(mut self) -> Self {
        self.call = format!("spawn_subagent model=\"{}\"", self.slug);
        self
    }
}

/// Query passed to the injected catalog backend.
#[derive(Debug, Clone)]
pub struct ModelCatalogQuery {
    pub query: String,
    pub limit: usize,
    pub provider: Option<String>,
    pub task_eligible_only: bool,
}
