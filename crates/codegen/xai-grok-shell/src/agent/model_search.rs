//! BM25 search over the live model catalog for `search_models`.
//!
//! Mirrors the ranking approach of `session/tool_index.rs` (exact short-circuit
//! + identifier decomposition + BM25) without embedding APIs.

use archanjo::{ModelCatalogQuery, SearchModelsHit, SearchModelsResult};
use bm25::{Language, SearchEngineBuilder};

use super::config::ModelEntry;
use super::model_providers::ModelProviderKind;
use super::models::{ModelsManager, is_task_agent_eligible};

/// Split a compound identifier into component words for BM25 matching.
fn split_identifier(s: &str) -> Vec<&str> {
    let mut words: Vec<&str> = Vec::new();
    for part in s
        .split(':')
        .flat_map(|p| p.split('/'))
        .flat_map(|p| p.split("__"))
        .flat_map(|p| p.split('_'))
        .flat_map(|p| p.split('-'))
        .flat_map(|p| p.split('.'))
    {
        if part.is_empty() {
            continue;
        }
        let bytes = part.as_bytes();
        let mut start = 0;
        for i in 1..bytes.len() {
            if bytes[i - 1].is_ascii_lowercase() && bytes[i].is_ascii_uppercase() {
                words.push(&part[start..i]);
                start = i;
            }
        }
        words.push(&part[start..]);
    }
    words
}

fn normalize_query(query: &str) -> String {
    let q = query.trim();
    if q.is_empty() {
        return String::new();
    }
    // Expand punctuation so "GLM 5.2" and "glm-5.2" share tokens.
    let spaced: String = q
        .chars()
        .map(|c| match c {
            ':' | '/' | '_' | '-' | '.' => ' ',
            other => other,
        })
        .collect();
    let needs_split = q.contains("__")
        || q.contains('_')
        || q.contains('-')
        || q.contains('.')
        || q.contains(':')
        || q.contains('/')
        || q.as_bytes()
            .windows(2)
            .any(|w| w[0].is_ascii_lowercase() && w[1].is_ascii_uppercase());
    if !needs_split {
        return spaced;
    }
    let extra: Vec<&str> = spaced
        .split_whitespace()
        .flat_map(split_identifier)
        .collect();
    if extra.is_empty() {
        return spaced;
    }
    format!("{spaced} {}", extra.join(" "))
}

fn provider_label(entry: &ModelEntry) -> String {
    if let Some(provider) = entry.model_provider.as_ref() {
        return match provider.kind {
            ModelProviderKind::OpenRouter => "openrouter".to_string(),
            ModelProviderKind::OpenAi => "openai".to_string(),
            ModelProviderKind::Anthropic => "anthropic".to_string(),
            ModelProviderKind::Xai => "xai".to_string(),
            ModelProviderKind::Zai => "zai".to_string(),
            ModelProviderKind::OpenAiCompatible => {
                if provider.id.is_empty() {
                    "openai_compatible".to_string()
                } else {
                    provider.id.clone()
                }
            }
        };
    }
    // First-party / no model_provider: treat as xai/builtin for filtering.
    "xai".to_string()
}

fn provider_instance_id(entry: &ModelEntry) -> String {
    crate::agent::config::provider_instance_id_for_entry(entry)
}

fn provider_kind_str(entry: &ModelEntry) -> String {
    entry
        .model_provider
        .as_ref()
        .map(|p| crate::agent::config::provider_kind_label(p.kind).to_string())
        .unwrap_or_else(|| provider_label(entry))
}

fn display_name(slug: &str, entry: &ModelEntry) -> String {
    entry
        .info
        .name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| slug.to_string())
}

fn to_document(slug: &str, entry: &ModelEntry, provider: &str) -> String {
    let name = display_name(slug, entry);
    let routing = entry.info.model.as_str();
    let instance = provider_instance_id(entry);
    let desc = entry.info.description.as_deref().unwrap_or("");
    let base = format!("{name} {slug} {routing} {provider} {instance} {desc}");
    let extra: String = [name.as_str(), slug, routing, provider, instance.as_str()]
        .into_iter()
        .flat_map(split_identifier)
        .collect::<Vec<_>>()
        .join(" ");
    format!("{base} {extra}")
}

fn provider_matches(entry_provider: &str, filter: &str) -> bool {
    let f = filter.trim().to_ascii_lowercase();
    if f.is_empty() {
        return true;
    }
    let p = entry_provider.to_ascii_lowercase();
    p == f
        || p.contains(&f)
        || f.contains(&p)
        // Accept common aliases.
        || (f == "open_router" && p == "openrouter")
        || (f == "open-router" && p == "openrouter")
}

struct CatalogRow {
    slug: String,
    entry: ModelEntry,
    provider: String,
    task_eligible: bool,
}

fn build_hit(row: &CatalogRow, score: Option<f32>) -> SearchModelsHit {
    let instance = provider_instance_id(&row.entry);
    let kind = provider_kind_str(&row.entry);
    let upstream = row.entry.info.model.clone();
    // Qualify display when instance differs from bare provider label so sibling
    // accounts sharing an upstream slug remain distinguishable in results.
    let bare = display_name(&row.slug, &row.entry);
    let name = if bare
        .to_ascii_lowercase()
        .contains(&instance.to_ascii_lowercase())
    {
        bare
    } else if instance != row.provider {
        format!("{bare} ({instance})")
    } else {
        bare
    };
    SearchModelsHit {
        name,
        slug: row.slug.clone(),
        provider: row.provider.clone(),
        provider_instance_id: Some(instance),
        provider_kind: Some(kind),
        upstream_model_id: Some(upstream),
        task_eligible: row.task_eligible,
        supports_tools: row.entry.info.supports_tools,
        context_window: Some(row.entry.info.context_window.get()),
        call: String::new(),
        score,
    }
    .with_call()
}

/// Search the current ModelsManager catalog.
pub fn search_models_catalog(
    manager: &ModelsManager,
    query: ModelCatalogQuery,
) -> SearchModelsResult {
    let is_session_auth = manager.is_session_auth();
    let models = manager.models();

    let mut rows: Vec<CatalogRow> = models
        .into_iter()
        .map(|(slug, entry)| {
            let provider = provider_label(&entry);
            let task_eligible = is_task_agent_eligible(&entry, is_session_auth);
            CatalogRow {
                slug,
                entry,
                provider,
                task_eligible,
            }
        })
        .collect();

    if query.task_eligible_only {
        rows.retain(|r| r.task_eligible);
    }
    if let Some(ref provider) = query.provider {
        rows.retain(|r| provider_matches(&r.provider, provider));
    }

    if rows.is_empty() {
        return SearchModelsResult {
            results: vec![],
            truncated: false,
            note: Some(
                if query.task_eligible_only {
                    "No task-eligible models are available. Connect a provider key or omit model on spawn to inherit the parent."
                } else {
                    "No models matched the filter."
                }
                .to_string(),
            ),
        };
    }

    let q = query.query.trim();
    if q.is_empty() {
        // Summary only — counts by provider, no full dump.
        let mut by_provider: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for r in &rows {
            *by_provider.entry(r.provider.clone()).or_default() += 1;
        }
        let summary = by_provider
            .iter()
            .map(|(p, n)| format!("{p}: {n}"))
            .collect::<Vec<_>>()
            .join(", ");
        return SearchModelsResult {
            results: vec![],
            truncated: false,
            note: Some(format!(
                "Pass a non-empty query to rank models. Catalog size: {} ({summary}).",
                rows.len()
            )),
        };
    }

    // Exact short-circuit: only when the match is unique. Multiple hits that
    // share an upstream slug / display name (sibling OpenAI vs OpenRouter
    // accounts) must remain distinguishable — never silently pick one.
    let exact_matches: Vec<&CatalogRow> = rows
        .iter()
        .filter(|r| {
            r.slug.eq_ignore_ascii_case(q)
                || r.entry.info.model.eq_ignore_ascii_case(q)
                || r.slug
                    .rsplit_once(':')
                    .is_some_and(|(_, rest)| rest.eq_ignore_ascii_case(q))
                || r.entry
                    .info
                    .name
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(q))
        })
        .collect();
    match exact_matches.len() {
        1 => {
            return SearchModelsResult {
                results: vec![build_hit(exact_matches[0], Some(1.0))],
                truncated: false,
                note: None,
            };
        }
        n if n > 1 => {
            // Prefer a unique exact-canonical-id hit if the query is a full slug.
            if let Some(canonical) = exact_matches
                .iter()
                .find(|r| r.slug.eq_ignore_ascii_case(q))
            {
                return SearchModelsResult {
                    results: vec![build_hit(canonical, Some(1.0))],
                    truncated: false,
                    note: None,
                };
            }
            // Ambiguous bare label / upstream slug: return all task-eligible
            // siblings with canonical slugs so the agent can pick explicitly.
            let mut results: Vec<SearchModelsHit> = exact_matches
                .into_iter()
                .map(|r| build_hit(r, Some(1.0)))
                .collect();
            results.sort_by(|a, b| a.slug.cmp(&b.slug));
            let truncated = results.len() > query.limit;
            results.truncate(query.limit);
            return SearchModelsResult {
                results,
                truncated,
                note: Some(
                    "Multiple models match that label; use the exact slug (provider-qualified) to select one."
                        .to_string(),
                ),
            };
        }
        _ => {}
    }

    let documents: Vec<String> = rows
        .iter()
        .map(|r| to_document(&r.slug, &r.entry, &r.provider))
        .collect();

    let search_engine =
        SearchEngineBuilder::<u32>::with_corpus(Language::English, documents).build();
    // Fetch extra candidates so limit still fills after any post-filter (none today).
    let fetch = query.limit.saturating_mul(2).max(query.limit).min(100);
    let normalized = normalize_query(q);
    let bm25_results = search_engine.search(&normalized, fetch);

    let mut results: Vec<SearchModelsHit> = bm25_results
        .into_iter()
        .filter_map(|sr| {
            let row = rows.get(sr.document.id as usize)?;
            Some(build_hit(row, Some(sr.score)))
        })
        .collect();

    let truncated = results.len() > query.limit;
    results.truncate(query.limit);

    let note = if results.is_empty() {
        Some(
            "No models matched the query. Try a shorter name (e.g. \"GLM 5.2\") or the provider prefix."
                .to_string(),
        )
    } else if truncated {
        Some(format!(
            "Showing top {} matches; refine the query for more precision.",
            query.limit
        ))
    } else {
        None
    };

    SearchModelsResult {
        results,
        truncated,
        note,
    }
}

impl ModelsManager {
    /// Agent-facing catalog search used by the `search_models` tool backend.
    pub fn search_models(&self, query: ModelCatalogQuery) -> SearchModelsResult {
        search_models_catalog(self, query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::ModelEntry;
    use crate::agent::model_providers::{ModelProviderKind, ResolvedModelProvider};
    use crate::auth::{AuthManager, GrokComConfig};
    use indexmap::IndexMap;
    use std::num::NonZeroU64;
    use std::sync::Arc;

    fn test_manager() -> ModelsManager {
        let tmp = std::env::temp_dir().join("grok-test-model-search");
        let auth_manager = Arc::new(AuthManager::new(&tmp, GrokComConfig::default()));
        ModelsManager::new(
            None,
            IndexMap::new(),
            agent_client_protocol::ModelId::new("default"),
            auth_manager,
            crate::agent::config::Config::default(),
        )
    }

    fn openrouter_entry(name: &str, routing: &str, tools: bool) -> ModelEntry {
        let mut info = crate::agent::config::ModelInfo::fallback(routing);
        info.name = Some(name.to_string());
        info.model = routing.to_string();
        info.supports_tools = Some(tools);
        info.user_selectable = true;
        info.context_window = NonZeroU64::new(131_072).unwrap();
        ModelEntry {
            info,
            model_provider: Some(ResolvedModelProvider {
                id: "openrouter".into(),
                kind: ModelProviderKind::OpenRouter,
                openrouter_fallback_models: vec![],
                openrouter_provider_preferences: None,
                openrouter_plugins: vec![],
                openrouter_pacing: false,
                command: vec![],
            }),
            api_key: Some("test-key".into()),
            env_key: None,
            auth_provider: None,
            api_base_url: None,
        }
    }

    fn seeded_manager() -> ModelsManager {
        let mgr = test_manager();
        mgr.insert_test_entry(
            "openrouter:z-ai/glm-5.2",
            openrouter_entry("Z.ai: GLM 5.2", "z-ai/glm-5.2", true),
        );
        mgr.insert_test_entry(
            "openrouter:z-ai/glm-5.1",
            openrouter_entry("Z.ai: GLM 5.1", "z-ai/glm-5.1", true),
        );
        mgr.insert_test_entry(
            "openrouter:openai/gpt-oss-120b",
            openrouter_entry("OpenAI: GPT OSS 120B", "openai/gpt-oss-120b", true),
        );
        mgr.insert_test_entry(
            "openrouter:no-tools/model",
            openrouter_entry("No Tools", "no-tools/model", false),
        );
        mgr
    }

    #[test]
    fn glm_query_ranks_glm_5_2() {
        let mgr = seeded_manager();
        let result = mgr.search_models(ModelCatalogQuery {
            query: "GLM 5.2".into(),
            limit: 5,
            provider: None,
            task_eligible_only: true,
        });
        assert!(
            !result.results.is_empty(),
            "expected hits, note={:?}",
            result.note
        );
        assert_eq!(result.results[0].slug, "openrouter:z-ai/glm-5.2");
        assert!(result.results[0].call.contains("openrouter:z-ai/glm-5.2"));
        assert!(
            !result
                .results
                .iter()
                .any(|h| h.slug == "openrouter:no-tools/model")
        );
    }

    #[test]
    fn gpt_oss_decomposition() {
        let mgr = seeded_manager();
        let result = mgr.search_models(ModelCatalogQuery {
            query: "gpt oss 120b".into(),
            limit: 5,
            provider: None,
            task_eligible_only: true,
        });
        assert!(
            result
                .results
                .iter()
                .any(|h| h.slug == "openrouter:openai/gpt-oss-120b"),
            "results={:?}",
            result.results
        );
    }

    #[test]
    fn exact_slug_short_circuit() {
        let mgr = seeded_manager();
        let result = mgr.search_models(ModelCatalogQuery {
            query: "openrouter:z-ai/glm-5.2".into(),
            limit: 5,
            provider: None,
            task_eligible_only: true,
        });
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.results[0].score, Some(1.0));
    }

    #[test]
    fn empty_query_summary_only() {
        let mgr = seeded_manager();
        let result = mgr.search_models(ModelCatalogQuery {
            query: "".into(),
            limit: 10,
            provider: None,
            task_eligible_only: true,
        });
        assert!(result.results.is_empty());
        assert!(
            result
                .note
                .as_deref()
                .unwrap_or("")
                .contains("Pass a non-empty")
        );
    }

    #[test]
    fn normalize_splits_versions() {
        let n = normalize_query("GLM 5.2");
        assert!(n.to_ascii_lowercase().contains('5'));
        assert!(n.to_ascii_lowercase().contains("glm"));
    }

    fn openai_entry(instance: &str, routing: &str, name: &str) -> ModelEntry {
        let mut info = crate::agent::config::ModelInfo::fallback(routing);
        info.name = Some(name.to_string());
        info.model = routing.to_string();
        info.supports_tools = Some(true);
        info.user_selectable = true;
        info.context_window = NonZeroU64::new(128_000).unwrap();
        ModelEntry {
            info,
            model_provider: Some(ResolvedModelProvider {
                id: instance.into(),
                kind: ModelProviderKind::OpenAi,
                openrouter_fallback_models: vec![],
                openrouter_provider_preferences: None,
                openrouter_plugins: vec![],
                openrouter_pacing: false,
                command: vec![],
            }),
            api_key: Some("test-key".into()),
            env_key: None,
            auth_provider: None,
            api_base_url: None,
        }
    }

    #[test]
    fn ambiguous_upstream_slug_returns_all_siblings_not_silent_pick() {
        let mgr = test_manager();
        mgr.insert_test_entry("openai:gpt-4o", openai_entry("openai", "gpt-4o", "GPT-4o"));
        mgr.insert_test_entry(
            "openai_work:gpt-4o",
            openai_entry("openai_work", "gpt-4o", "GPT-4o"),
        );
        let result = mgr.search_models(ModelCatalogQuery {
            query: "gpt-4o".into(),
            limit: 10,
            provider: None,
            task_eligible_only: true,
        });
        assert!(
            result.results.len() >= 2,
            "expected both siblings, got {:?}",
            result.results
        );
        let slugs: Vec<&str> = result.results.iter().map(|h| h.slug.as_str()).collect();
        assert!(slugs.contains(&"openai:gpt-4o"));
        assert!(slugs.contains(&"openai_work:gpt-4o"));
        assert!(
            result
                .note
                .as_deref()
                .unwrap_or("")
                .contains("Multiple models match"),
            "note={:?}",
            result.note
        );
        // Structured instance metadata present and secret-free.
        for hit in &result.results {
            assert!(hit.provider_instance_id.is_some());
            assert!(hit.upstream_model_id.as_deref() == Some("gpt-4o"));
            assert!(!hit.slug.contains("test-key"));
            assert!(!format!("{hit:?}").contains("test-key"));
        }
    }

    #[test]
    fn exact_canonical_slug_short_circuits_even_with_sibling_upstream() {
        let mgr = test_manager();
        mgr.insert_test_entry("openai:gpt-4o", openai_entry("openai", "gpt-4o", "GPT-4o"));
        mgr.insert_test_entry(
            "openai_work:gpt-4o",
            openai_entry("openai_work", "gpt-4o", "GPT-4o"),
        );
        let result = mgr.search_models(ModelCatalogQuery {
            query: "openai_work:gpt-4o".into(),
            limit: 5,
            provider: None,
            task_eligible_only: true,
        });
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.results[0].slug, "openai_work:gpt-4o");
        assert_eq!(
            result.results[0].provider_instance_id.as_deref(),
            Some("openai_work")
        );
    }
}
