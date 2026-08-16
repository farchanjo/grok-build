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

/// Reference-impact / preliminary remove readiness (PR13 owns final forced remove).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceImpactSnapshot {
    pub provider_id: String,
    pub generation: RegistryGeneration,
    pub can_remove: bool,
    pub blocked_reason: Option<String>,
    pub model_references: Vec<String>,
    pub session_pin_hints: Vec<String>,
    pub cache_present: bool,
    pub secrets_present: bool,
    pub guidance: String,
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
