//! Types for the Archanjo `search_models` tool.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input for the `search_models` tool.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SearchModelsInput {
    /// Free-text query such as a product name, version, or slug fragment
    /// (e.g. `"GLM 5.2"`, `"gpt-oss-120b"`, `"openrouter:z-ai/glm-5.2"`).
    /// Empty query returns a short provider summary only — not the full catalog.
    #[serde(default)]
    pub query: String,
    /// Maximum number of ranked results (default 10, max 30).
    #[serde(default)]
    pub limit: Option<u32>,
    /// Optional provider filter (e.g. `"openrouter"`, `"openai"`, `"xai"`, `"codex"`).
    /// Matching is case-insensitive against the provider id/kind.
    #[serde(default)]
    pub provider: Option<String>,
    /// When true (default), only return models that can be used as
    /// `spawn_subagent` / `Task.model` (credentialed, visible, tool-capable).
    #[serde(default = "default_task_eligible_only")]
    pub task_eligible_only: Option<bool>,
}

fn default_task_eligible_only() -> Option<bool> {
    Some(true)
}

impl SearchModelsInput {
    pub fn limit_or_default(&self) -> usize {
        self.limit.unwrap_or(10).clamp(1, 30) as usize
    }

    pub fn task_eligible_only_or_default(&self) -> bool {
        self.task_eligible_only.unwrap_or(true)
    }
}

/// One catalog hit returned to the model.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SearchModelsHit {
    /// Human-readable display name (label).
    pub name: String,
    /// Catalog key for `spawn_subagent` `model=` (e.g. `openrouter:z-ai/glm-5.2`).
    pub slug: String,
    /// Provider id/kind string (e.g. `openrouter`, `xai`).
    pub provider: String,
    /// Whether this slug passes the same gate as `Task.model` validation.
    pub task_eligible: bool,
    /// Catalog tools flag when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tools: Option<bool>,
    /// Context window when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
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
