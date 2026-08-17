//! Named retrieval configuration: parse, validate, management, notify.
//!
//! Shell-authoritative graph for embedding models, rerankers, retrieval
//! profiles, prime consumers, and optional memory profile selection.
//! Credentials and provider connection details stay in `[model_providers.*]`.
//!
//! PR15 scope: configuration + validation + durable management + reverse
//! references. No network adapters, runtime orchestration, or reindex execution.

pub mod management;
pub mod notify;
pub mod parse;
pub mod toml_edit;
pub mod validate;

/// Secret-free management DTOs (re-exported for pager).
pub mod dto {
    pub use super::management::dto::*;
}

pub use management::{
    RetrievalManagementService,
    dto::{
        EmbeddingModelDto, MemoryReindexImpact, PrimeDto, RerankerModelDto, RetrievalConflictInfo,
        RetrievalGraphSnapshot, RetrievalMutationResult, RetrievalPreviewResult,
        RetrievalProfileDto,
    },
};
pub use parse::{ParsedRetrievalGraph, parse_retrieval_graph};
pub use validate::{
    GraphValidationIssue, ProviderCapabilityView, validate_retrieval_graph,
    validate_retrieval_graph_with_providers,
};
pub use xai_grok_config_types::{
    AgentPrimeConfig, EmbeddingEncoding, EmbeddingModelConfig, EmbeddingProtocol, PrimeConfig,
    RerankerModelConfig, RerankerProtocol, RetrievalFallbackStrategy, RetrievalGraphConfig,
    RetrievalProfileConfig, SkillPrimeConfig,
};
// Re-export management generation tag (shared shape with provider registry).
pub use crate::provider_registry::management::dto::RegistryGeneration;
