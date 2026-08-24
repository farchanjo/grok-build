//! `search_models` — discover task-eligible catalog models for `spawn_subagent`.
//!
//! Backend is injected by the shell (`ModelCatalogSearch`) so this crate stays
//! free of `ModelsManager` / provider types. Ranking (BM25) lives in the shell.

mod types;

pub use types::{ModelCatalogQuery, SearchModelsHit, SearchModelsInput, SearchModelsResult};

use std::sync::Arc;

use xai_grok_tools::types::output::ToolOutput;
use xai_grok_tools::types::tool::{ToolKind, ToolNamespace};
use xai_grok_tools::types::tool_io::ToolInput;
use xai_grok_tools::types::tool_metadata::ToolMetadata;

/// Injected catalog search backend (shell implements with ModelsManager + BM25).
type SearchModelsFn = dyn Fn(ModelCatalogQuery) -> SearchModelsResult + Send + Sync;

#[derive(Clone)]
pub struct ModelCatalogSearch(Arc<SearchModelsFn>);

impl ModelCatalogSearch {
    pub fn new(
        search: impl Fn(ModelCatalogQuery) -> SearchModelsResult + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(search))
    }

    pub fn search(&self, query: ModelCatalogQuery) -> SearchModelsResult {
        (self.0)(query)
    }
}

impl std::fmt::Debug for ModelCatalogSearch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelCatalogSearch").finish()
    }
}

xai_grok_tools::register_resource!("archanjo", "ModelCatalogSearch", ModelCatalogSearch);

const DESCRIPTION: &str = r#"Search available model catalog entries for subagent spawning.

Use when the user names a model by product name or version (e.g. "GLM 5.2",
"gpt-oss-120b") and you need the exact catalog slug for `spawn_subagent` `model=`.

Returns ranked hits with: name, slug, provider, task_eligible, and a `call`
example. Pass the **slug** field exactly as `model` — do not invent slugs.

If the user does not request a model, omit `model` on spawn to inherit the parent.
Empty query returns a short provider summary only (not the full catalog)."#;

/// Archanjo catalog search tool (`Archanjo:search_models`).
#[derive(Debug, Default)]
pub struct SearchModelsTool;

impl ToolMetadata for SearchModelsTool {
    fn kind(&self) -> ToolKind {
        ToolKind::SearchModels
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::Archanjo
    }

    fn description_template(&self) -> &str {
        DESCRIPTION
    }
}

impl From<SearchModelsInput> for ToolInput {
    fn from(input: SearchModelsInput) -> Self {
        // Out-of-tree packs map through Dynamic so core ToolInput stays free
        // of custom pack type dependencies.
        match serde_json::to_value(input) {
            Ok(value) => ToolInput::Dynamic(value),
            Err(_) => ToolInput::Dynamic(serde_json::json!({})),
        }
    }
}

impl xai_tool_runtime::Tool for SearchModelsTool {
    type Args = SearchModelsInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("search_models").expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "search_models",
            ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: true,
            tool_scope: Some(xai_tool_protocol::ToolScope::Read),
            ..Default::default()
        }
    }

    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: SearchModelsInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use xai_grok_tools::types::tool_metadata::shared_resources;
        let resources = shared_resources(&ctx)?;

        let Some(catalog) = resources.lock().await.get::<ModelCatalogSearch>().cloned() else {
            let payload = SearchModelsResult {
                results: vec![],
                truncated: false,
                note: Some(
                    "Model catalog search is not available in this session (backend not injected)."
                        .to_string(),
                ),
            };
            return Ok(ToolOutput::Text(format_result(&payload).into()));
        };

        let limit = input.limit_or_default();
        let task_eligible_only = input.task_eligible_only_or_default();
        let result = catalog.search(ModelCatalogQuery {
            query: input.query,
            limit,
            provider: input.provider,
            task_eligible_only,
        });

        tracing::info!(
            result_count = result.results.len() as u32,
            truncated = result.truncated,
            "archanjo.search_models.search"
        );

        Ok(ToolOutput::Text(format_result(&result).into()))
    }
}

fn format_result(result: &SearchModelsResult) -> String {
    if let Ok(pretty) = serde_json::to_string_pretty(result) {
        return pretty;
    }
    format!("results={}", result.results.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_tools::types::resources::Resources;
    use xai_grok_tools::types::tool_metadata::test_ctx;

    #[tokio::test]
    async fn missing_backend_returns_note() {
        let resources = Resources::default().into_shared();
        let out = xai_tool_runtime::Tool::run(
            &SearchModelsTool,
            test_ctx(resources),
            SearchModelsInput {
                query: "GLM 5.2".into(),
                limit: Some(5),
                provider: None,
                task_eligible_only: Some(true),
            },
        )
        .await
        .expect("ok");
        let ToolOutput::Text(text) = out else {
            panic!("expected Text");
        };
        assert!(text.text.contains("not available") || text.text.contains("backend"));
        assert!(text.text.contains("\"results\""));
    }

    #[tokio::test]
    async fn injected_backend_returns_hits() {
        let mut resources = Resources::default();
        resources.insert(ModelCatalogSearch::new(|q| {
            assert_eq!(q.query, "GLM 5.2");
            assert!(q.task_eligible_only);
            SearchModelsResult {
                results: vec![
                    SearchModelsHit {
                        name: "Z.ai: GLM 5.2".into(),
                        slug: "openrouter:z-ai/glm-5.2".into(),
                        provider: "openrouter".into(),
                        provider_instance_id: Some("openrouter".into()),
                        provider_kind: Some("openrouter".into()),
                        upstream_model_id: Some("z-ai/glm-5.2".into()),
                        task_eligible: true,
                        supports_tools: Some(true),
                        context_window: Some(131072),
                        call: String::new(),
                        score: Some(1.0),
                    }
                    .with_call(),
                ],
                truncated: false,
                note: None,
            }
        }));
        let out = xai_tool_runtime::Tool::run(
            &SearchModelsTool,
            test_ctx(resources.into_shared()),
            SearchModelsInput {
                query: "GLM 5.2".into(),
                limit: None,
                provider: None,
                task_eligible_only: None,
            },
        )
        .await
        .expect("ok");
        let ToolOutput::Text(text) = out else {
            panic!("expected Text");
        };
        assert!(text.text.contains("openrouter:z-ai/glm-5.2"));
        assert!(text.text.contains("spawn_subagent model="));
        assert!(text.text.contains("Z.ai: GLM 5.2"));
    }

    #[test]
    fn tool_id_kind_and_namespace() {
        let t = SearchModelsTool;
        assert_eq!(xai_tool_runtime::Tool::id(&t).as_str(), "search_models");
        assert_eq!(ToolMetadata::kind(&t), ToolKind::SearchModels);
        assert_eq!(ToolMetadata::tool_namespace(&t), ToolNamespace::Archanjo);
        assert!(ToolMetadata::is_read_only(&t));
        assert_eq!(
            format!(
                "{}:{}",
                ToolMetadata::tool_namespace(&t),
                xai_tool_runtime::Tool::id(&t).as_str()
            ),
            "Archanjo:search_models"
        );
    }
}
