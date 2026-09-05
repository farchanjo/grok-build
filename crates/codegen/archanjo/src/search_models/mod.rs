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

const DESCRIPTION: &str = r#"Search the live model catalog to resolve a product name or version into an exact `slug` for `spawn_subagent` / `task` `model=` (e.g. "GLM 5.2" — "openrouter:z-ai/glm-5.2").

When to use:
- The user names a model by product name, version, or slug fragment and you do not know the exact catalog slug.
- You need to see what a provider offers or compare candidates before spawning.

When not to use:
- The user did not name a model — omit `model` and inherit the parent.
- You already know the exact slug.

How to use:
1. Pass a short query (name/version, e.g. "GLM 5.2"); long queries rank worse.
2. Optional `provider`: "openrouter", "openai", "xai", "anthropic", "zai", or an instance id.
3. `task_eligible_only` defaults to true (spawn-able models only); set false to see the whole catalog.
4. Pass the returned `slug` exactly as `model` — never invent or modify it. The `call` field shows the exact spawn invocation.

Hits are structured objects with:
- Identity: name, slug, provider, provider_instance_id, provider_kind, upstream_model_id, description.
- Capabilities: supports_tools, supports_image_input, supports_audio_input, supports_video_input, supports_file_input, output_has_text. Each modal field is tri-state: true, false, or null when the catalog is silent. Null means unknown, NOT supported.
- Limits: context_window, max_completion_tokens, max_output_ceiling, supports_zdr.
- Reasoning: supports_reasoning_effort, reasoning_efforts.
- Spawn decision: task_eligible (same gate as Task.model validation), call.

Caveats:
- Only task_eligible hits can be spawned; a model without tool support is rejected by the spawn tool.
- "openai:"-prefixed entries are discovery rows and are never spawn-able.
- Empty query returns a provider summary only, not the full catalog."#;

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
        use xai_grok_tools::types::output::SearchModelsOutput;
        use xai_grok_tools::types::tool_metadata::shared_resources;
        let resources = shared_resources(&ctx)?;

        let content = |result: &SearchModelsResult| -> String {
            serde_json::to_string_pretty(result).unwrap_or_else(|_| {
                format!("results={}", result.results.len())
            })
        };

        let Some(catalog) = resources.lock().await.get::<ModelCatalogSearch>().cloned() else {
            let payload = SearchModelsResult {
                results: vec![],
                truncated: false,
                note: Some(
                    "Model catalog search is not available in this session (backend not injected)."
                        .to_string(),
                ),
            };
            let content = content(&payload);
            return Ok(ToolOutput::SearchModels(SearchModelsOutput {
                results: payload.results,
                truncated: payload.truncated,
                note: payload.note,
                content,
            }));
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

        let content = content(&result);
        Ok(ToolOutput::SearchModels(SearchModelsOutput {
            results: result.results,
            truncated: result.truncated,
            note: result.note,
            content,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_tools::types::output::SearchModelsOutput;
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
        let ToolOutput::SearchModels(out) = out else {
            panic!("expected SearchModels");
        };
        assert!(out.content.contains("not available") || out.content.contains("backend"));
        assert!(out.content.contains("\"results\""));
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
                        description: Some("Z.ai GLM 5.2 model".into()),
                        task_eligible: true,
                        supports_tools: Some(true),
                        supports_image_input: Some(true),
                        supports_audio_input: None,
                        supports_video_input: None,
                        supports_file_input: Some(true),
                        output_has_text: Some(true),
                        context_window: Some(131072),
                        max_completion_tokens: Some(8192),
                        max_output_ceiling: Some(8192),
                        supports_zdr: None,
                        supports_native_schema: None,
                        supports_strict_tools: None,
                        supports_reasoning_effort: Some(true),
                        reasoning_efforts: vec!["low".into(), "medium".into(), "high".into()],
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
        let ToolOutput::SearchModels(out) = out else {
            panic!("expected SearchModels");
        };
        assert_eq!(out.results.len(), 1);
        let hit = &out.results[0];
        assert_eq!(hit.slug, "openrouter:z-ai/glm-5.2");
        assert_eq!(hit.supports_image_input, Some(true));
        assert_eq!(hit.supports_audio_input, None);
        assert_eq!(hit.reasoning_efforts, vec!["low", "medium", "high"]);
        assert!(hit.call.contains("spawn_subagent model="));
        assert!(out.content.contains("openrouter:z-ai/glm-5.2"));
        assert!(out.content.contains("Z.ai: GLM 5.2"));
    }

    /// Full-field round-trip through the typed payload: every field must
    /// survive the serialized `content` the model reads.
    #[test]
    fn typed_output_round_trips_full_fields() {
        let hit = SearchModelsHit {
            name: "Z.ai: GLM 5.2".into(),
            slug: "openrouter:z-ai/glm-5.2".into(),
            provider: "openrouter".into(),
            provider_instance_id: Some("openrouter".into()),
            provider_kind: Some("openrouter".into()),
            upstream_model_id: Some("z-ai/glm-5.2".into()),
            description: Some("Z.ai GLM 5.2 model".into()),
            task_eligible: true,
            supports_tools: Some(true),
            supports_image_input: Some(true),
            supports_audio_input: Some(false),
            supports_video_input: None,
            supports_file_input: Some(true),
            output_has_text: Some(true),
            context_window: Some(131072),
            max_completion_tokens: Some(8192),
            max_output_ceiling: Some(8192),
            supports_zdr: Some(true),
            supports_native_schema: None,
            supports_strict_tools: Some(false),
            supports_reasoning_effort: Some(true),
            reasoning_efforts: vec!["low".into(), "high".into()],
            call: String::new(),
            score: Some(1.0),
        }
        .with_call();
        let payload = SearchModelsOutput {
            results: vec![hit],
            truncated: false,
            note: None,
            content: String::new(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        let reparsed: SearchModelsOutput = serde_json::from_value(json).unwrap();
        assert_eq!(reparsed.results.len(), 1);
        assert_eq!(reparsed.results[0].upstream_model_id.as_deref(), Some("z-ai/glm-5.2"));
        assert_eq!(reparsed.results[0].supports_zdr, Some(true));
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
