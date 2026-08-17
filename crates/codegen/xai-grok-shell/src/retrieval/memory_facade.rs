//! Credential-free memory retrieval facade over the PR17 `RetrievalService`.
//!
//! PR21 routes memory embedding + optional remote reranking through the exact
//! named PR15 profile and PR17's exact provider-instance/incarnation routes —
//! never the active chat model base URL, API key, OAuth, or credential
//! provider. This facade is **credential-free**: it holds only the service
//! handle, the profile id, and the resolved non-secret source spec.
//!
//! When no named profile is configured, the shell synthesizes a deterministic
//! legacy source (from `[memory.embedding]` + legacy endpoint behavior) so
//! existing users do not reconnect, rewrite config, or rebuild unnecessarily.

use std::sync::Arc;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use xai_grok_config_types::EmbeddingEncoding;
use xai_grok_inference::DEFAULT_EMBEDDINGS_PATH;
use xai_grok_memory::{EmbeddingSourceSpec, RetrievalError, RetrievalErrorKind};

use super::error::OrchestratorError;
use super::pipeline::PipelineOptions;
use super::service::RetrievalService;

/// Max docs forwarded to the remote reranker (bounded; final truncation is
/// applied by the memory search pipeline).
const RERANK_DOC_CAP: usize = 64;

/// Credential-free `xai_grok_memory::MemoryRetrieval` over `RetrievalService`.
pub struct RetrievalServiceMemoryFacade {
    service: RetrievalService,
    profile_id: String,
    source_spec: EmbeddingSourceSpec,
    /// Whether the profile has any reranker route (skip remote rerank calls
    /// entirely when absent).
    has_rerank_routes: bool,
}

impl std::fmt::Debug for RetrievalServiceMemoryFacade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetrievalServiceMemoryFacade")
            .field("profile_id", &self.profile_id)
            .field("source", &self.source_spec)
            .field("has_rerank_routes", &self.has_rerank_routes)
            .finish()
    }
}

impl RetrievalServiceMemoryFacade {
    /// Resolve the named profile against the live snapshot and build a
    /// credential-free facade. Returns `None` when the profile is absent,
    /// disabled, or has no usable embedding route (callers then degrade to
    /// legacy/FTS-only — never a sibling-credential fallback).
    pub fn new(service: &RetrievalService, profile_id: &str) -> Option<Self> {
        let snapshot = service.load_snapshot();
        if !snapshot.enabled {
            return None;
        }
        let profile = snapshot.profile(profile_id)?;
        let emb_id = profile.embedding_route_ids.first()?;
        let emb = snapshot.embedding_route(emb_id)?;
        let dimensions = emb.config.dimensions.unwrap_or(0) as usize;
        if dimensions == 0 {
            return None;
        }
        let protocol = emb.config.protocol.as_str();
        let encoding: &str = match emb.config.encoding {
            EmbeddingEncoding::Float => "float",
            EmbeddingEncoding::Base64 => "base64",
        };
        let source_spec = EmbeddingSourceSpec {
            provider_instance_id: emb.provider_instance_id.clone(),
            incarnation: emb.incarnation.clone(),
            origin_host: emb.origin_host.clone(),
            embedding_path: DEFAULT_EMBEDDINGS_PATH.to_owned(),
            protocol: protocol.to_owned(),
            model: emb.config.model.clone(),
            dimensions,
            encoding: encoding.to_owned(),
            normalization: xai_grok_memory::fingerprint::NORMALIZATION_NONE.to_owned(),
        };
        Some(Self {
            service: service.clone(),
            profile_id: profile_id.to_owned(),
            source_spec,
            has_rerank_routes: !profile.reranker_route_ids.is_empty(),
        })
    }

    /// Profile id (diagnostics).
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    /// The resolved source spec (credential-free).
    pub fn source_spec(&self) -> &EmbeddingSourceSpec {
        &self.source_spec
    }

    fn options() -> PipelineOptions {
        PipelineOptions {
            hard_error_on_semantic_failure: false,
            bypass_semantic: false,
            pin_snapshot_generation: None,
            hard_error_on_limit_exceeded: false,
        }
    }
}

fn map_error(e: &OrchestratorError) -> RetrievalError {
    let kind = match e {
        OrchestratorError::ServiceDisabled | OrchestratorError::ProfileMissing { .. } => {
            RetrievalErrorKind::SourceUnavailable
        }
        OrchestratorError::Cancelled { .. } => RetrievalErrorKind::Cancelled,
        OrchestratorError::DeadlineExceeded { .. }
        | OrchestratorError::AttemptBudgetExceeded { .. }
        | OrchestratorError::InputBudgetExceeded { .. }
        | OrchestratorError::OutputBudgetExceeded { .. }
        | OrchestratorError::LimitExceeded { .. } => RetrievalErrorKind::BudgetExhausted,
        _ => RetrievalErrorKind::Transient,
    };
    RetrievalError::new(kind)
}

#[async_trait]
impl xai_grok_memory::MemoryRetrieval for RetrievalServiceMemoryFacade {
    fn source_spec(&self) -> EmbeddingSourceSpec {
        self.source_spec.clone()
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, RetrievalError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let options = Self::options();
        match self
            .service
            .embed(
                &self.profile_id,
                texts.to_vec(),
                options,
                CancellationToken::new(),
            )
            .await
        {
            Ok(stage) => {
                // Validate dimensionality — never hand a mixed/foreign space
                // vector to the memory index (pinned space check is upstream
                // in the provider adapter as well).
                let mut out = Vec::with_capacity(stage.result.vectors.len());
                for v in &stage.result.vectors {
                    if v.values.len() != self.source_spec.dimensions {
                        return Err(RetrievalError::new(RetrievalErrorKind::Malformed));
                    }
                    out.push(v.values.clone());
                }
                if out.len() != texts.len() {
                    return Err(RetrievalError::new(RetrievalErrorKind::Malformed));
                }
                Ok(out)
            }
            Err(e) => Err(map_error(&e)),
        }
    }

    async fn rerank(
        &self,
        query: &str,
        documents: &[String],
    ) -> Result<Option<Vec<usize>>, RetrievalError> {
        if !self.has_rerank_routes || documents.is_empty() {
            return Ok(None);
        }
        let docs: Vec<String> = documents.iter().take(RERANK_DOC_CAP).cloned().collect();
        let options = Self::options();
        match self
            .service
            .rerank(
                &self.profile_id,
                query.to_owned(),
                docs.clone(),
                options,
                CancellationToken::new(),
            )
            .await
        {
            Ok(stage) => match stage.result {
                Some(result) => {
                    // The caller validates the permutation (perfect, all
                    // indices in range); any invalid result restores the
                    // complete exact local pre-rerank order.
                    Ok(Some(result.hits.iter().map(|h| h.index).collect()))
                }
                None => Ok(None),
            },
            Err(e) => {
                tracing::warn!(
                    profile_id = %self.profile_id,
                    error = ?map_error(&e),
                    "memory remote rerank unavailable; keeping exact local pre-rerank order"
                );
                Ok(None)
            }
        }
    }
}

/// Build the memory retrieval facade for a named profile (PR15).
///
/// Returns `None` when the profile cannot be resolved — the caller degrades to
/// legacy/FTS-only behavior.
pub fn facade_for_profile(
    service: &RetrievalService,
    profile_id: &str,
) -> Option<Arc<dyn xai_grok_memory::MemoryRetrieval>> {
    RetrievalServiceMemoryFacade::new(service, profile_id)
        .map(|f| -> Arc<dyn xai_grok_memory::MemoryRetrieval> { Arc::new(f) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::clients::FakeRetrievalExecutor;
    use crate::retrieval::registry::RetrievalRegistry;
    use crate::retrieval::reload::{
        SnapshotBuildInput, test_graph_two_embed_routes, test_provider_views_capable,
    };
    use crate::retrieval::service::RetrievalService;
    use std::sync::Arc;
    use xai_grok_memory::MemoryRetrieval as _;

    fn service_with_graph() -> (RetrievalService, Arc<FakeRetrievalExecutor>) {
        let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
        let reg = RetrievalRegistry::disabled("/tmp/pr21-mem-facade-test");
        reg.publish_build_input(
            0,
            SnapshotBuildInput {
                graph: test_graph_two_embed_routes(),
                graph_generation: 1,
                provider_generation: 1,
                provider_views: views,
                provider_meta: meta,
                parse_warnings: Vec::new(),
            },
        );
        let executor = Arc::new(FakeRetrievalExecutor::new());
        let service = reg.service().with_executor(executor.clone());
        (service, executor)
    }

    /// The named profile routes embedding through the profile's exact
    /// provider instance/incarnation — never the active chat model base URL,
    /// API key, OAuth, or credential provider.
    #[tokio::test]
    async fn named_profile_route_is_independent_of_chat_credentials() {
        let (service, executor) = service_with_graph();
        let facade = RetrievalServiceMemoryFacade::new(&service, "default").unwrap();
        assert_eq!(facade.source_spec().provider_instance_id, "acct-a");
        assert_eq!(facade.source_spec().model, "embed-a");
        assert_eq!(facade.source_spec().dimensions, 8);
        assert_eq!(
            facade.source_spec().incarnation.as_deref(),
            Some("inc-acct-a")
        );

        // Embed through the facade; the fake executor records which provider
        // instance actually served the call.
        let vectors = facade
            .embed_batch(&["hello".to_owned(), "world".to_owned()])
            .await
            .unwrap();
        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors[0].len(), 8);
        assert_eq!(
            executor.embed_calls(),
            vec!["emb-a".to_owned()],
            "must route through the profile's primary embedding model"
        );
        assert_eq!(
            executor.provider_ids_seen(),
            vec!["acct-a".to_owned()],
            "must route through the profile's exact provider instance (not the chat provider)"
        );
    }

    /// Reranker-only changes never affect the embedding source spec, so no
    /// vector rebuild is triggered.
    #[test]
    fn reranker_only_change_does_not_alter_source_spec() {
        let (service, _executor) = service_with_graph();
        let with_rr = RetrievalServiceMemoryFacade::new(&service, "default").unwrap();

        let mut graph = test_graph_two_embed_routes();
        // Remove the reranker entirely (reranker-only change).
        graph.reranker_models.clear();
        graph
            .retrieval_profiles
            .get_mut("default")
            .unwrap()
            .reranker_models
            .clear();
        let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
        let reg = RetrievalRegistry::disabled("/tmp/pr21-mem-facade-test-2");
        reg.publish_build_input(
            0,
            SnapshotBuildInput {
                graph,
                graph_generation: 1,
                provider_generation: 1,
                provider_views: views,
                provider_meta: meta,
                parse_warnings: Vec::new(),
            },
        );
        let service2 = reg.service();
        let without_rr = RetrievalServiceMemoryFacade::new(&service2, "default").unwrap();
        assert_eq!(
            with_rr.source_spec(),
            without_rr.source_spec(),
            "reranker-only changes must not alter the embedding source identity"
        );
        assert!(with_rr.has_rerank_routes);
        assert!(!without_rr.has_rerank_routes);
    }

    /// Unknown/missing profile => None (callers degrade to legacy/FTS-only).
    #[test]
    fn unknown_profile_degrades_to_none() {
        let (service, _executor) = service_with_graph();
        assert!(RetrievalServiceMemoryFacade::new(&service, "missing-profile").is_none());
    }

    /// Rerank via the profile returns hit indices; outages degrade to None
    /// (the memory caller then keeps its exact local pre-rerank order).
    #[tokio::test]
    async fn facade_rerank_returns_indices_or_none() {
        let (service, executor) = service_with_graph();
        let facade = RetrievalServiceMemoryFacade::new(&service, "default").unwrap();
        let perm = facade
            .rerank("q", &["d0".to_owned(), "d1".to_owned(), "d2".to_owned()])
            .await
            .unwrap();
        // Default FakeRerankScript::Ok returns hits in document order.
        assert_eq!(perm, Some(vec![0usize, 1, 2]));
        assert_eq!(executor.rerank_calls().len(), 1);
    }

    /// Debug output of the facade never includes credentials/vectors/text.
    #[test]
    fn facade_debug_is_credential_free() {
        let (service, _executor) = service_with_graph();
        let facade = RetrievalServiceMemoryFacade::new(&service, "default").unwrap();
        let dbg = format!("{facade:?}");
        assert!(dbg.contains("profile_id"));
        assert!(dbg.contains("acct-a"));
        assert!(!dbg.contains("sk-"));
        assert!(!dbg.contains("bearer"));
    }
}
