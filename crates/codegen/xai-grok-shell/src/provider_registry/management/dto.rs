//! Secret-free provider management DTOs for shell + pager.
//!
//! These types are safe for reducer `Debug`, logs, snapshots, and task results.
//! They never carry API keys, OAuth tokens, admin tokens, or env values.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Monotonic lifecycle generation for the durable provider registry.
///
/// Every list/detail snapshot and every save request is tagged. Stale async
/// results and stale saves fail closed (reload / retry / clone guidance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct RegistryGeneration(pub u64);

impl RegistryGeneration {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Credential slot presence (never values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CredentialPresence {
    pub has_application_key: bool,
    pub has_admin_key: bool,
    pub has_oauth: bool,
    /// Application key generation when known (binding generation, not the secret).
    pub application_generation: Option<u64>,
    pub admin_generation: Option<u64>,
    pub oauth_generation: Option<u64>,
}

/// One row on the providers browse list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderListRow {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub enabled: bool,
    pub is_built_in: bool,
    pub is_configured: bool,
    pub base_url: Option<String>,
    pub credentials: CredentialPresence,
    /// Short connection status for the list (shell-authored, secret-free).
    pub status_label: String,
    pub status_detail: Option<String>,
}

/// Full typed detail for the editor (all feature-owned fields, no secrets).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderDetailDto {
    pub id: String,
    pub display_name: Option<String>,
    pub kind: String,
    pub enabled: bool,
    pub is_built_in: bool,
    pub is_configured: bool,
    pub is_editable: bool,
    pub base_url: Option<String>,
    pub admin_base_url: Option<String>,
    pub default_backend: Option<String>,
    pub auth_scheme: Option<String>,
    pub env_key: Option<String>,
    pub admin_env_key: Option<String>,
    pub catalog_enabled: bool,
    pub capability_mode: Option<String>,
    pub catalog_ttl_secs: Option<u64>,
    pub request_timeout_secs: Option<u64>,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub api_surface: Option<String>,
    pub credential_route: Option<String>,
    pub api_backend: Option<String>,
    pub auth_provider: Option<String>,
    pub extra_headers: IndexMap<String, String>,
    pub capabilities: IndexMap<String, bool>,
    /// OpenRouter policy (feature-owned fields).
    pub openrouter_fallback_models: Vec<String>,
    pub openrouter_data_collection: Option<String>,
    pub openrouter_require_parameters: Option<bool>,
    pub openrouter_allow_fallbacks: Option<bool>,
    pub openrouter_zdr: Option<bool>,
    pub openrouter_order: Vec<String>,
    pub openrouter_only: Vec<String>,
    pub openrouter_ignore: Vec<String>,
    pub openrouter_quantizations: Vec<String>,
    pub openrouter_sort: Option<String>,
    pub openrouter_pacing: bool,
    /// Plugin ids only (safe labels; full plugin JSON stays in config).
    pub openrouter_plugin_ids: Vec<String>,
    pub credentials: CredentialPresence,
    pub generation: RegistryGeneration,
    pub warnings: Vec<String>,
    /// Unsupported-edit fail-closed note when kind is unknown/legacy.
    pub unsupported_edit_reason: Option<String>,
    /// Grok-owned lifecycle incarnation (UUID), when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<String>,
    /// True when a tombstone blocks ordinary re-add of this id.
    #[serde(default)]
    pub tombstone_blocks_readd: bool,
}

/// Browse snapshot returned by the shell management service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderListSnapshot {
    pub generation: RegistryGeneration,
    pub rows: Vec<ProviderListRow>,
    pub warnings: Vec<String>,
}

/// Secret field semantics for saves: empty means preserve; explicit clear.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SecretFieldUpdate {
    /// Leave the durable secret unchanged.
    #[default]
    Preserve,
    /// Replace with a new value (carried only in one-shot redacted effects, never
    /// in durable DTO state). The shell accepts this only through secure paths.
    Set,
    /// Explicitly clear the slot.
    Clear,
}

/// Credential write request (secret values travel only via redacted one-shot
/// channels, never inside this DTO's Debug-safe fields).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CredentialSlotUpdate {
    pub application: SecretFieldUpdate,
    pub admin: SecretFieldUpdate,
    pub oauth: SecretFieldUpdate,
}

/// Typed save patch for General / Auth metadata / Catalog / Capabilities /
/// Headers / OpenRouter Policy pages. Secrets are separate.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderSavePatch {
    pub display_name: Option<String>,
    pub kind: Option<String>,
    pub base_url: Option<String>,
    pub admin_base_url: Option<String>,
    pub enabled: Option<bool>,
    pub default_backend: Option<String>,
    pub auth_scheme: Option<String>,
    pub env_key: Option<String>,
    pub admin_env_key: Option<String>,
    pub catalog_enabled: Option<bool>,
    pub capability_mode: Option<String>,
    pub catalog_ttl_secs: Option<u64>,
    pub request_timeout_secs: Option<u64>,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub api_surface: Option<String>,
    pub credential_route: Option<String>,
    pub extra_headers: Option<IndexMap<String, String>>,
    pub capabilities: Option<IndexMap<String, bool>>,
    pub openrouter_fallback_models: Option<Vec<String>>,
    pub openrouter_data_collection: Option<Option<String>>,
    pub openrouter_require_parameters: Option<Option<bool>>,
    pub openrouter_allow_fallbacks: Option<Option<bool>>,
    pub openrouter_zdr: Option<Option<bool>>,
    pub openrouter_order: Option<Vec<String>>,
    pub openrouter_only: Option<Vec<String>>,
    pub openrouter_ignore: Option<Vec<String>>,
    pub openrouter_quantizations: Option<Vec<String>>,
    pub openrouter_sort: Option<Option<String>>,
    pub openrouter_pacing: Option<bool>,
}

/// Add-provider request (new configured instance).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderAddRequest {
    pub id: String,
    pub kind: String,
    pub base_url: String,
    pub display_name: Option<String>,
    pub admin_base_url: Option<String>,
    pub enabled: bool,
    pub expected_generation: RegistryGeneration,
}

/// Clone request: copy metadata from `source_id` into a new id (secrets never cloned).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderCloneRequest {
    pub source_id: String,
    pub new_id: String,
    pub display_name: Option<String>,
    pub expected_generation: RegistryGeneration,
}

/// Save request for an existing provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderSaveRequest {
    pub id: String,
    pub expected_generation: RegistryGeneration,
    pub patch: ProviderSavePatch,
}

/// Result of a durable mutation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderMutationResult {
    pub ok: bool,
    pub id: String,
    pub generation: RegistryGeneration,
    pub error: Option<String>,
    /// When generation was stale, shell guidance for the client.
    pub stale: bool,
    pub guidance: Option<String>,
    /// True when config and/or secrets were already durable but generation
    /// bookkeeping failed or had to be force-reconciled. Clients must reload.
    #[serde(default)]
    pub partial_commit: bool,
    /// Live incarnation after the mutation (when known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<String>,
    /// Client operation id echo for late-async discard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    /// Stale multi-client conflict (safe field names only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<ProviderConflictInfo>,
    /// Safe field names changed by this mutation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_fields: Vec<String>,
}

/// Catalog summary for the Catalog page (secret-free).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogStatusSnapshot {
    pub provider_id: String,
    pub generation: RegistryGeneration,
    pub catalog_enabled: bool,
    pub model_count: Option<usize>,
    pub last_refresh_label: Option<String>,
    pub source: Option<String>,
    pub error: Option<String>,
    pub sample_model_ids: Vec<String>,
}

/// Capability profile snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityStatusSnapshot {
    pub provider_id: String,
    pub generation: RegistryGeneration,
    pub mode: String,
    pub capabilities: IndexMap<String, bool>,
    pub discovered: IndexMap<String, bool>,
    pub error: Option<String>,
}

/// Connection / status probe result (real, not placeholder success).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderStatusSnapshot {
    pub provider_id: String,
    pub generation: RegistryGeneration,
    pub connected: bool,
    pub label: String,
    pub detail: Option<String>,
    pub error: Option<String>,
}

/// Credits probe (OpenRouter getCredits when applicable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderCreditsSnapshot {
    pub provider_id: String,
    pub generation: RegistryGeneration,
    pub available: bool,
    pub summary: Option<String>,
    pub error: Option<String>,
}

/// Group kind for reverse-reference impact (secret-free labels only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactGroupKind {
    ModelsAndDefaults,
    Sessions,
    AgentsAndSubagents,
    WorkflowsAndGoals,
    AuxiliaryRoutes,
    Memory,
    /// Structurally empty until PR15 named retrieval config.
    RetrievalProfiles,
    EmbeddingModels,
    RerankerModels,
}

impl ImpactGroupKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::ModelsAndDefaults => "Models & defaults",
            Self::Sessions => "Sessions",
            Self::AgentsAndSubagents => "Agents & subagents",
            Self::WorkflowsAndGoals => "Workflows & goals",
            Self::AuxiliaryRoutes => "Compaction / media / web / suggestion",
            Self::Memory => "Memory",
            Self::RetrievalProfiles => "Retrieval profiles",
            Self::EmbeddingModels => "Embedding models",
            Self::RerankerModels => "Reranker models",
        }
    }
}

/// One secret-free reverse reference row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactReference {
    pub kind: ImpactGroupKind,
    pub label: String,
}

/// Grouped reverse references for the References page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactGroup {
    pub kind: ImpactGroupKind,
    pub references: Vec<ImpactReference>,
}

/// Reference-impact / remove readiness with grouped durable & active refs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceImpactSnapshot {
    pub provider_id: String,
    pub generation: RegistryGeneration,
    pub can_remove: bool,
    pub blocked_reason: Option<String>,
    /// Compatibility flat list (model definitions). Prefer `groups`.
    pub model_references: Vec<String>,
    /// Compatibility flat list (session pins). Prefer `groups`.
    pub session_pin_hints: Vec<String>,
    pub cache_present: bool,
    pub secrets_present: bool,
    pub guidance: String,
    /// Grouped reverse references (bounded, secret-free).
    #[serde(default)]
    pub groups: Vec<ImpactGroup>,
    /// Fail-closed scan diagnostics (never secrets).
    #[serde(default)]
    pub scan_errors: Vec<String>,
    /// True when any group hit its bound.
    #[serde(default)]
    pub truncated: bool,
    /// Disable excludes from new selection and blocks the next request/turn.
    #[serde(default = "default_true")]
    pub disable_blocks_next_turn: bool,
}

fn default_true() -> bool {
    true
}

/// Optional credential/cache clear choices for forced remove (never implicit).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ForceRemoveClearOptions {
    pub clear_application_key: bool,
    pub clear_admin_key: bool,
    pub clear_oauth: bool,
    pub clear_catalog_cache: bool,
    pub clear_capability_cache: bool,
}

/// Forced remove request: requires exact typed provider id barrier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderForceRemoveRequest {
    pub id: String,
    /// Client must type the exact provider id (case-sensitive).
    pub typed_id_confirmation: String,
    pub expected_generation: RegistryGeneration,
    /// Live incarnation when known (reject when mismatched).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_incarnation: Option<String>,
    pub clear: ForceRemoveClearOptions,
    /// Client operation id for late-async discard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// Safe conflict payload for stale multi-client edits (field names only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConflictInfo {
    pub provider_id: String,
    pub client_generation: RegistryGeneration,
    pub live_generation: RegistryGeneration,
    /// Safe changed field names only — never secrets or raw values.
    pub changed_fields: Vec<String>,
    pub guidance: String,
}

/// Extended mutation result fields for PR13 async safety.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderMutationMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<ProviderConflictInfo>,
    /// Safe field names touched by this mutation (for leader broadcast).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_fields: Vec<String>,
}

/// Editor page identity (keyboard navigation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderEditorPage {
    General,
    Authentication,
    Catalog,
    Capabilities,
    Headers,
    OpenRouterPolicy,
    References,
}

impl ProviderEditorPage {
    pub const ALL: [Self; 7] = [
        Self::General,
        Self::Authentication,
        Self::Catalog,
        Self::Capabilities,
        Self::Headers,
        Self::OpenRouterPolicy,
        Self::References,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Authentication => "Authentication",
            Self::Catalog => "Catalog",
            Self::Capabilities => "Capabilities",
            Self::Headers => "Headers",
            Self::OpenRouterPolicy => "OpenRouter Policy",
            Self::References => "References",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or(Self::General)
    }
}
