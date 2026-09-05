//! Types for the Archanjo `search_models` tool.
//!
//! The wire contract (hits/result/query) is re-exported from the shared leaf
//! (`xai-tool-types`) so `xai-grok-tools` and the pack reference one
//! definition. The tool *input* stays local: it is the tool's `Args` and the
//! pack owns the `From<SearchModelsInput> for ToolInput` conversion (orphan
//! rule — both types would otherwise be foreign).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use xai_tool_types::search_models::{
    ModelCatalogQuery, SearchModelsHit, SearchModelsResult,
};

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
    /// Optional provider filter (e.g. `"openrouter"`, `"openai"`, `"xai"`, `"anthropic"`,
    /// `"zai"`). Matching is case-insensitive against the provider id/kind.
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
