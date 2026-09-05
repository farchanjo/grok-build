//! Secret-free retrieval graph management DTOs for shell + pager.
//!
//! Never carry API keys, OAuth tokens, endpoints with credentials, or env values.
//! Safe for reducer `Debug`, logs, snapshots, toasts, and task results.

use crate::provider_registry::management::dto::RegistryGeneration;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use xai_grok_config_types::{
    AgentPrimeConfig, EmbeddingModelConfig, PrimeConfig, RerankerModelConfig,
    RetrievalProfileConfig, SkillPrimeConfig,
};

/// Embedding model row/detail (credential-free).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingModelDto {
    pub id: String,
    pub config: EmbeddingModelConfig,
}

/// Reranker model row/detail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RerankerModelDto {
    pub id: String,
    pub config: RerankerModelConfig,
}

/// Retrieval profile row/detail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalProfileDto {
    pub id: String,
    pub config: RetrievalProfileConfig,
}

/// Prime aggregate DTO.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PrimeDto {
    pub skills: SkillPrimeConfig,
    pub agents: AgentPrimeConfig,
    /// Optional `[prime] vector_store` selection; carried so TUI drafts
    /// preserve the live value across save.
    pub vector_store: Option<String>,
}

impl From<PrimeConfig> for PrimeDto {
    fn from(p: PrimeConfig) -> Self {
        Self {
            skills: p.skills,
            agents: p.agents,
            vector_store: p.vector_store,
        }
    }
}

/// Full retrieval graph snapshot (shell-authoritative).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RetrievalGraphSnapshot {
    pub generation: RegistryGeneration,
    pub embedding_models: Vec<EmbeddingModelDto>,
    pub reranker_models: Vec<RerankerModelDto>,
    pub retrieval_profiles: Vec<RetrievalProfileDto>,
    pub prime: PrimeDto,
    /// Optional `[memory] retrieval_profile`.
    pub memory_retrieval_profile: Option<String>,
    /// Memory mode: "local" or "milvus".
    #[serde(default = "default_memory_mode_str")]
    pub memory_mode: String,
    /// Optional `[memory] vector_store`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_vector_store: Option<String>,
    /// Structured warning messages (secret-free).
    pub warnings: Vec<String>,
    /// Hard validation issues (paths + messages).
    pub validation_errors: Vec<String>,
    /// Soft validation notes.
    pub validation_warnings: Vec<String>,
    /// Whether the graph is currently valid for save (no hard errors).
    pub is_valid: bool,
}

fn default_memory_mode_str() -> String {
    "local".into()
}

/// Multi-client conflict info (safe field names only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalConflictInfo {
    pub client_generation: RegistryGeneration,
    pub live_generation: RegistryGeneration,
    /// Safe field / section names that changed.
    pub changed_fields: Vec<String>,
    pub guidance: String,
}

/// Result of a durable graph mutation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalMutationResult {
    pub ok: bool,
    pub generation: RegistryGeneration,
    pub error: Option<String>,
    pub stale: bool,
    pub guidance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<RetrievalConflictInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_fields: Vec<String>,
    /// Client operation id echo for late-async discard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    /// Memory reindex impact when embedding identity/dimensions changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_reindex: Option<MemoryReindexImpact>,
    /// Snapshot after successful mutation (optional; clients may reload).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<RetrievalGraphSnapshot>,
}

/// Memory reindex impact (computed only; never executed in PR15).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryReindexImpact {
    /// True when selected embedding identity/dimensions would change.
    pub requires_confirmation: bool,
    /// Human-readable reason (secret-free).
    pub reason: String,
    /// Prior fingerprint label when known.
    pub previous_fingerprint: Option<String>,
    /// New fingerprint label when known.
    pub next_fingerprint: Option<String>,
}

/// Bounded synthetic validation / preview (network-free, credential-free).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalPreviewResult {
    pub generation: RegistryGeneration,
    /// True when config is structurally ready; never pretends a provider call occurred.
    pub validation_ready: bool,
    pub messages: Vec<String>,
    pub operation_id: Option<String>,
}

/// Request to replace the entire retrieval graph (save).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalGraphSaveRequest {
    pub expected_generation: RegistryGeneration,
    pub embedding_models: IndexMap<String, EmbeddingModelConfig>,
    pub reranker_models: IndexMap<String, RerankerModelConfig>,
    pub retrieval_profiles: IndexMap<String, RetrievalProfileConfig>,
    pub prime: PrimeConfig,
    pub memory_retrieval_profile: Option<String>,
    /// Explicit confirmation that memory reindex impact was accepted.
    #[serde(default)]
    pub confirm_memory_reindex: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// Upsert a single embedding model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpsertEmbeddingRequest {
    pub expected_generation: RegistryGeneration,
    pub id: String,
    pub config: EmbeddingModelConfig,
    #[serde(default)]
    pub confirm_memory_reindex: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// Upsert a single reranker model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpsertRerankerRequest {
    pub expected_generation: RegistryGeneration,
    pub id: String,
    pub config: RerankerModelConfig,
    #[serde(default)]
    pub confirm_memory_reindex: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// Upsert a single retrieval profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpsertProfileRequest {
    pub expected_generation: RegistryGeneration,
    pub id: String,
    pub config: RetrievalProfileConfig,
    #[serde(default)]
    pub confirm_memory_reindex: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// Clone request (new id; source id immutable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CloneRetrievalEntityRequest {
    pub expected_generation: RegistryGeneration,
    /// `embedding` | `reranker` | `profile`
    pub kind: String,
    pub source_id: String,
    pub new_id: String,
    #[serde(default)]
    pub confirm_memory_reindex: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// Delete request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteRetrievalEntityRequest {
    pub expected_generation: RegistryGeneration,
    /// `embedding` | `reranker` | `profile`
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub confirm_memory_reindex: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// Reorder request (new full ordered id list for a section).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReorderRetrievalRequest {
    pub expected_generation: RegistryGeneration,
    /// `embedding` | `reranker` | `profile`
    pub kind: String,
    pub ordered_ids: Vec<String>,
    #[serde(default)]
    pub confirm_memory_reindex: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// Prime save request with reindex confirm flag (prime cannot change embedding
/// identity, but confirm is threaded for uniform mutation discipline).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavePrimeRequest {
    pub expected_generation: RegistryGeneration,
    pub prime: PrimeConfig,
    #[serde(default)]
    pub confirm_memory_reindex: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// Memory profile selection save.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveMemoryProfileRequest {
    pub expected_generation: RegistryGeneration,
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector_store: Option<String>,
    #[serde(default)]
    pub confirm_memory_reindex: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}
