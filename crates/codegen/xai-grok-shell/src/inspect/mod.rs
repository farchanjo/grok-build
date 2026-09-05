//! `grok inspect` — configuration introspection.
//!
//! Shows everything Grok discovers in the current directory: project
//! instructions, permissions, hooks, skills, agents, plugins, MCP servers,
//! LSP config, and config.toml sources. Supports `--json` for machine output.

mod compat;

pub use compat::{CompatEntryStatus, CompatSource, ExternalCompatEntry, ExternalCompatReport};
use compat::{
    derive_vendor, instruction_compat_status, resolve_inspect_compat, vendor_compat_status,
    vendor_tag,
};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize, Serializer};
use xai_grok_memory::workspace_storage_identity;
use xai_grok_tools::types::config_source::ConfigSource;
use xai_grok_tools::util::truncate::estimate_tokens;

use crate::auth::ForceLoginTeam;

fn serialize_config_source_label<S: Serializer>(
    source: &ConfigSource,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&source.display_label())
}

/// Basename-only label for inspect presentation. Never an absolute, UNC, or
/// home path — JSON and human output share this contract. Splits on both
/// `/` and `\` so Windows-style fixtures stay basename-only on Unix hosts.
fn inspect_path_label(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("file")
        .to_string()
}

fn serialize_inspect_path_label<S: Serializer>(
    path: &str,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&inspect_path_label(path))
}

fn serialize_optional_inspect_path_label<S: Serializer>(
    path: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match path {
        Some(p) => serialize_inspect_path_label(p, serializer),
        None => serializer.serialize_none(),
    }
}

/// Command basename, or empty for URLs / empty values. Never a host or path.
fn inspect_target_label(raw: &str) -> String {
    if raw.is_empty() || raw.contains("://") {
        return String::new();
    }
    raw.rsplit(['/', '\\'])
        .next()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("")
        .to_string()
}

fn inspect_requirement_source_label(
    src: &xai_grok_workspace::permission::types::RequirementSource,
) -> String {
    use xai_grok_workspace::permission::types::RequirementSource::*;
    match src {
        Unknown => "<unknown>".into(),
        Requirements { .. } => "requirements".into(),
        SystemRequirements { .. } => "system-requirements".into(),
        ManagedSettings { .. } => "managed-settings".into(),
        ManagedConfig { .. } => "managed-config".into(),
        Config { .. } => "config".into(),
        Settings { .. } => "settings".into(),
    }
}

fn serialize_inspect_target<S: Serializer>(target: &str, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&inspect_target_label(target))
}

fn inspect_fingerprint_short(fp: &str) -> String {
    fp.chars().take(12).collect()
}

const TREE: &str = "\u{2514}";

/// Coarse scope label for project instructions and plugin entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Project,
    User,
    Global,
    Plugin,
    Builtin,
    Cli,
    Config,
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Project => "project",
            Self::User => "user",
            Self::Global => "global",
            Self::Plugin => "plugin",
            Self::Builtin => "builtin",
            Self::Cli => "cli",
            Self::Config => "config",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectReport {
    pub grok_version: String,
    pub channel: String,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
    /// Folder-trust verdict for `cwd`: when false, repo-local project hooks,
    /// plugins, and MCP/LSP entries are gated out of the listings below.
    pub project_trusted: bool,
    pub project_instructions: Vec<InstructionFile>,
    pub permissions: PermissionsReport,
    pub login_policy: LoginPolicyReport,
    pub hooks: Vec<HookEntry>,
    pub skills: Vec<SkillEntry>,
    pub agents: Vec<AgentEntry>,
    pub plugins: Vec<PluginEntry>,
    pub marketplaces: Vec<MarketplaceEntry>,
    pub mcp_servers: Vec<McpServerEntry>,
    pub lsp_servers: Vec<LspServerEntry>,
    pub config_sources: ConfigSources,
    pub external_compat: ExternalCompatReport,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub config_warnings: Vec<crate::agent::config_model_override_parse::ConfigWarning>,
    /// Provider instance registry overview (secret-free).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_registry: Option<ProviderRegistryInspect>,
    /// Model catalog overview on the API-key-auth visibility view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_catalog: Option<ModelCatalogInspect>,
    /// Published retrieval/prime/memory runtime plus disk-candidate status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieval: Option<RetrievalInspect>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionFile {
    #[serde(serialize_with = "serialize_inspect_path_label")]
    pub path: String,
    pub scope: Scope,
    pub file_type: String,
    pub size_bytes: usize,
    /// Estimated token count (chars / 4).
    pub approx_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// True when this entry's vendor surface is disabled by compat config.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility_status: Option<CompatEntryStatus>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsReport {
    pub sources: Vec<String>,
    pub loaded: usize,
    pub skipped: Vec<SkippedRule>,
    pub mcp_server_allowlist: Vec<String>,
    pub marketplace_allowlist: Vec<String>,
    /// Platform path for managed-settings.json vendor policy (None on unsupported OS).
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_inspect_path_label"
    )]
    pub managed_settings_path: Option<String>,
    /// Whether that file exists on disk. Always emitted, so a JSON consumer
    /// can distinguish "absent" from "present" without string-matching.
    pub managed_settings_exists: bool,
    /// Whether the runtime actually loaded that file into policy (`exists` can
    /// be true while this is false for an unreadable/malformed file). Always emitted.
    pub managed_settings_active: bool,
    /// Settings forced by a policy layer.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub enforced: Vec<EnforcedPolicy>,
}

/// One policy-enforced setting. Structured for `--json`; the human view
/// derives its line from these fields (see `enforced_label`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnforcedPolicy {
    /// Stable key: "alwaysApprove" | "telemetry" | "feedback".
    pub setting: String,
    /// The enforced value.
    pub enabled: bool,
    /// Originating file, e.g. "managed-settings.json".
    pub source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedRule {
    pub rule: String,
    pub reason: String,
}

/// Enterprise login-hardening policy resolved from `[grok_com_config]`
/// (TOML + env). Surfaced so admins can verify the deployment loaded it.
/// The team pin is admin policy, not a secret, so it is shown verbatim.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPolicyReport {
    /// Raw `disable_api_key_auth` knob (env `GROK_DISABLE_API_KEY_AUTH`).
    pub disable_api_key_auth: Option<bool>,
    /// Configured team pin: single string, list, or null when unset.
    pub force_login_team_uuid: Option<ForceLoginTeam>,
    /// Resolved verdict — true when either knob forces first-party login.
    pub api_key_auth_disabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookEntry {
    pub event: String,
    pub hook_type: String,
    #[serde(
        skip_serializing_if = "String::is_empty",
        serialize_with = "serialize_inspect_target"
    )]
    pub target: String,
    #[serde(serialize_with = "serialize_config_source_label")]
    pub source: ConfigSource,
    pub matcher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// True when this entry's vendor surface is disabled by compat config.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility_status: Option<CompatEntryStatus>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    #[serde(serialize_with = "serialize_config_source_label")]
    pub source: ConfigSource,
    pub user_invocable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// True when disabled by `[skills].disabled` config or when this entry's
    /// vendor surface is disabled by compat config.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility_status: Option<CompatEntryStatus>,
    /// Compact non-enableable quarantine row. Codes only; no body or path.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub quarantined: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostic_codes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEntry {
    pub name: String,
    pub description: String,
    #[serde(serialize_with = "serialize_config_source_label")]
    pub source: ConfigSource,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEntry {
    pub name: String,
    pub scope: Scope,
    #[serde(serialize_with = "serialize_inspect_path_label")]
    pub path: String,
    pub enabled: bool,
    pub provides: PluginProvides,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginProvides {
    pub skills: usize,
    pub agents: usize,
    pub hooks: bool,
    pub mcp_servers: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceEntry {
    pub name: String,
    #[serde(serialize_with = "serialize_inspect_path_label")]
    pub path: String,
    pub enabled_plugins: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerEntry {
    pub name: String,
    pub transport: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub target: String,
    #[serde(serialize_with = "serialize_config_source_label")]
    pub source: ConfigSource,
    /// True when this entry's vendor surface is disabled by compat config.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility_status: Option<CompatEntryStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LspServerEntry {
    pub name: String,
    #[serde(serialize_with = "serialize_inspect_path_label")]
    pub command: String,
    pub args: Vec<String>,
    #[serde(serialize_with = "serialize_config_source_label")]
    pub source: ConfigSource,
    pub extensions: Vec<String>,
    /// True when this project-scoped server would be skipped (untrusted folder).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub untrusted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSources {
    /// Config layers (system + user managed, user + system requirements, user
    /// config.toml, the macOS MDM managed-preferences layer, and project
    /// .grok/config.toml files). Driven from the same resolvers used at runtime
    /// (`ConfigLayers`, `requirements_layers`) so system + MDM layers and
    /// precedence are included, and emptiness reflects real contribution after
    /// stripping (version_overrides, fail_closed, etc).
    pub layers: Vec<ConfigLayer>,
}

/// A single config layer entry for `grok inspect`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigLayer {
    /// Logical role of the layer: "system-managed", "managed", "user",
    /// "system-requirements", "requirements", "mdm", or "project".
    pub role: String,
    #[serde(serialize_with = "serialize_inspect_path_label")]
    pub path: String,
    /// "empty" or "parse error" when the on-disk file does not contribute
    /// effective config (after the real loader's processing). Omitted when
    /// the layer is present and contributes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

// ── providerRegistry ────────────────────────────────────────────────────────

/// Provider instance registry overview (report `providerRegistry`). Built
/// only from the authoritative production `ProviderService` / cache /
/// management snapshot. Credential presence is rendered as a bounded status
/// enum; no env name/value, header, URL, binding, or raw cache payload is
/// serialized.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRegistryInspect {
    pub generation: u64,
    pub providers: Vec<ProviderRowInspect>,
    /// Bounded label (`unavailable`, `invalid`) when the runtime could not be
    /// read; other sections are unaffected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRowInspect {
    pub id: String,
    /// Full non-secret local lifecycle identity; human output shortens it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<String>,
    pub kind: String,
    /// Ordered, de-duplicated API-surface names.
    pub api_surfaces: Vec<String>,
    /// Ordered, de-duplicated credential-route names.
    pub credential_routes: Vec<String>,
    pub credential_status: CredentialStatus,
    pub enabled: bool,
    pub is_built_in: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tombstoned: Option<bool>,
    pub registry_generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog: Option<CacheSummaryInspect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability: Option<CapabilitySummaryInspect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<ReferenceInspect>,
}

/// Safe credential status label. Never a connection verdict and never a
/// credential value/name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStatus {
    Configured,
    Environment,
    Oauth,
    Helper,
    Missing,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheSummaryInspect {
    pub validity: CacheValidity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetched_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_generation: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySummaryInspect {
    pub validity: CacheValidity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetched_label: Option<String>,
}

/// Cache / catalog / capability validity label (never an endpoint/binding
/// fingerprint value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheValidity {
    Valid,
    Mismatch,
    Corrupt,
    Tombstoned,
    Unavailable,
    NotChecked,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceInspect {
    pub can_remove: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    /// Grouped reference kind -> count only (never reference labels/names).
    pub groups: Vec<ReferenceGroupInspect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceGroupInspect {
    pub kind: String,
    pub count: usize,
}

// ── model catalog ───────────────────────────────────────────────────────────

/// Model catalog overview on the API-key-auth visibility view (report
/// `modelCatalog`). Credential presence may be read to compute the visible
/// count, but credential values are never returned; OAuth-only entries are
/// excluded.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogInspect {
    /// 0 in a one-shot inspect (the live publication generation is owned by a
    /// running process). `modelsCache` documents whether disk contributed or
    /// why a present record was rejected.
    pub generation: u64,
    pub total_visible_count: usize,
    /// Always `"api_key"`; documented for JSON consumers.
    pub auth_view: String,
    /// `valid` | `absent` | `stale` | `mismatch` | `corrupt` | `unavailable`.
    pub models_cache: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<ModelDefaultInspect>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<ModelRowInspect>,
    /// Overlapping upstream slugs grouped by upstream, canonical ids sorted.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub duplicate_groups: Vec<DuplicateGroupInspect>,
    /// Deterministic alias enumeration, sorted by input.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<AliasRecordInspect>,
    /// Additional-account keys omitted by the multi-account rollout kill-switch.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub gated_entries: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDefaultInspect {
    /// Canonical selection id.
    pub id: String,
    /// Bounded source label from the authoritative default resolver
    /// (`xai_grok_config_types::ConfigSource`, serialized via its Display).
    pub source: String,
    pub upstream: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRowInspect {
    pub canonical_id: String,
    pub upstream_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub origin: String,
    pub user_selectable: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroupInspect {
    pub upstream_id: String,
    pub canonical_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasRecordInspect {
    pub input: String,
    /// `exact` | `permanentCompatibility` | `uniqueLegacy` | `ambiguous` | `gated` | `missing`.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
}

// ── retrieval & prime ───────────────────────────────────────────────────────

/// Retrieval runtime plus disk-candidate status (report `retrieval`). `source`
/// is `published` only when a live process registry exists; one-shot inspect
/// uses `disk`. An invalid candidate is always `validity=invalid`, and
/// `lastKnownGoodRetained=true` only when a live enabled snapshot proves it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalInspect {
    pub source: String,
    pub validity: RetrievalValidity,
    pub generation: u64,
    pub graph_generation: u64,
    pub provider_generation: u64,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    pub embedding_models: Vec<EmbeddingRouteInspect>,
    pub reranker_models: Vec<RerankerRouteInspect>,
    pub profiles: Vec<ProfileInspect>,
    pub prime: PrimeConfigInspect,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_retrieval_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_pin: Option<MemoryPinInspect>,
    /// Remote vector-store mirrors resolved in this process (memory and
    /// prime collections). Secret-free: backend label, lifecycle state, and
    /// row count only — no URI, token, or fingerprint hash.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub vector_mirrors: Vec<VectorMirrorInspect>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_known_good_retained: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
    /// Compact Prime index snapshot (truncated fingerprints, no vectors/paths).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prime_index: Option<PrimeIndexInspect>,
}

/// Inspect projection of the Prime metadata index. Truncated fingerprints only.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimeIndexInspect {
    pub generation: u64,
    pub fingerprint_short: String,
    pub skills_items: u64,
    pub skills_vectors: u64,
    pub skills_readiness: String,
    pub agents_items: u64,
    pub agents_vectors: u64,
    pub agents_readiness: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_route: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalValidity {
    Valid,
    Invalid,
    Disabled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingRouteInspect {
    pub id: String,
    pub provider_instance_id: String,
    /// Full non-secret local lifecycle identity for exact route correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<String>,
    pub model: String,
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    pub encoding: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RerankerRouteInspect {
    pub id: String,
    pub provider_instance_id: String,
    /// Full non-secret local lifecycle identity for exact route correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<String>,
    pub model: String,
    pub protocol: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInspect {
    pub id: String,
    pub embedding_route_ids: Vec<String>,
    pub reranker_route_ids: Vec<String>,
    pub budgets: ProfileBudgetsInspect,
    pub fallback_strategy: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileBudgetsInspect {
    pub deadline_ms: u64,
    pub max_attempts: u32,
    pub max_input_tokens: u32,
    pub max_output_tokens: u32,
    pub max_candidates: u32,
    pub max_results: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimeConfigInspect {
    pub skills: PrimeSectionInspect,
    pub agents: PrimeSectionInspect,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimeSectionInspect {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieval_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_total_chars: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_context_fraction: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline_ms: Option<u64>,
    pub degrade_on_error: bool,
}

/// Credential-free embedding source persisted with the installed vectors.
#[derive(Debug, Clone, Deserialize)]
struct InstalledMemorySource {
    provider_instance_id: String,
    incarnation: Option<String>,
    origin_host: String,
    embedding_path: String,
    protocol: String,
    model: String,
    dimensions: usize,
    encoding: String,
}

#[derive(Debug, Clone, Deserialize)]
struct InstalledMemoryFingerprint {
    source: InstalledMemorySource,
}

/// Secret-free live mirror state for `/context` (see
/// `crate::session::vector_mirror`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorMirrorInspect {
    /// Backend label (`milvus`).
    pub backend: String,
    /// Remote collection name (contains no secrets — workspace identity
    /// hash prefix + kind).
    pub collection: String,
    /// Lifecycle state: `syncing` | `ready` | `unavailable`.
    pub state: String,
    /// Row count last reported by the mirror, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<u64>,
}

/// Installed memory-vector identity, emitted only when no rebuild is pending.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPinInspect {
    pub configured_profile: String,
    pub installed_provider_instance_id: String,
    pub installed_model: String,
    pub installed_protocol: String,
    pub configured_route_matches_installed: bool,
    /// First 12 chars of the non-secret embedding-space hash.
    pub embedding_space_fingerprint_short: String,
    pub pinned_until_rebuild_or_new_session: bool,
}

pub async fn inspect(cwd: &Path, json: bool) -> anyhow::Result<()> {
    let report = build_report(cwd).await;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_human(&report);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Inspect section builders. Each takes `home` so tests use isolated fixtures.
// All sources are the authoritative production services/parsers/snapshots; no
// second parser and no state writes.
// ---------------------------------------------------------------------------

/// Upper bound for reported warning/diagnostic list length.
const MAX_INSPECT_WARNINGS: usize = 8;
/// Upper bound for name/id lists (aliases, gated entries).
const MAX_NAMES: usize = 8;
/// Upper bound for a diagnostic/reason string.
const MAX_DIAG_LEN: usize = 160;
const MAX_MEMORY_FINGERPRINT_PAYLOAD_BYTES: usize = 16 * 1024;
const MEMORY_FINGERPRINT_HASH_LEN: usize = 32;

/// Deterministic order-preserving dedup of route/surface labels.
fn ordered_dedup(iter: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in iter {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

fn bound(s: &str) -> String {
    let mut chars = s.chars();
    let taken: String = chars.by_ref().take(MAX_DIAG_LEN).collect();
    if chars.next().is_some() {
        format!("{taken}\u{2026}")
    } else {
        taken
    }
}

fn bound_warns<'a>(iter: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    iter.into_iter()
        .take(MAX_INSPECT_WARNINGS)
        .map(bound)
        .collect()
}

fn bound_reasons(reasons: Vec<String>) -> String {
    let parts: Vec<String> = reasons.into_iter().take(3).map(|r| bound(&r)).collect();
    if parts.is_empty() {
        "invalid".to_owned()
    } else {
        parts.join("; ")
    }
}

fn build_provider_registry(home: &Path) -> ProviderRegistryInspect {
    use crate::provider_registry::{ProviderRef, runtime_cache};
    match runtime_cache::load_runtime(home) {
        Ok((service, lifecycle, generation)) => {
            let mgmt = crate::provider_registry::ProviderManagementService::new(home);
            let mut providers = Vec::new();
            for desc in service.list() {
                let id = desc.id.as_str().to_owned();
                let meta = service.snapshot().get(&id);
                let presence = mgmt.credential_presence(&id);
                let api_surfaces = ordered_dedup(
                    desc.routes
                        .iter()
                        .map(|r| r.api_surface.as_str().to_owned()),
                );
                let credential_routes = ordered_dedup(
                    desc.routes
                        .iter()
                        .map(|r| r.credential_route.as_str().to_owned()),
                );
                let tombstoned = lifecycle.has_blocking_tombstone_for_id(&id)
                    || desc
                        .incarnation
                        .as_ref()
                        .is_some_and(|inc| lifecycle.is_tombstoned(&id, inc));
                let (catalog, capability) = build_cache_capability(home, &id, desc);
                let references = build_references(&mgmt, &id);
                providers.push(ProviderRowInspect {
                    id: id.clone(),
                    incarnation: desc.incarnation.as_ref().map(|i| i.as_str().to_owned()),
                    kind: desc.kind.as_str().to_owned(),
                    api_surfaces,
                    credential_routes,
                    credential_status: derive_credential_status(desc, meta, &presence),
                    enabled: desc.enabled,
                    is_built_in: matches!(desc.provider_ref, ProviderRef::BuiltIn(_)),
                    tombstoned: Some(tombstoned),
                    registry_generation: generation,
                    catalog,
                    capability,
                    references,
                });
            }
            ProviderRegistryInspect {
                generation,
                providers,
                diagnostic: None,
            }
        }
        Err(_) => ProviderRegistryInspect {
            generation: 0,
            providers: Vec::new(),
            diagnostic: Some("unavailable".to_owned()),
        },
    }
}

/// Strict allowlist credential-status derivation (see plan §2.1 table).
fn derive_credential_status(
    desc: &crate::provider_registry::ProviderInstanceDescriptor,
    meta: Option<&crate::provider_registry::ProviderMetadata>,
    presence: &crate::provider_registry::CredentialPresence,
) -> CredentialStatus {
    use crate::provider_registry::CredentialRoute;
    if presence.has_oauth {
        return CredentialStatus::Oauth;
    }
    let has_env_credential = desc
        .env_keys
        .iter()
        .any(|name| std::env::var(name).is_ok_and(|value| !value.trim().is_empty()))
        || meta.is_some_and(|m| {
            [m.env_key.as_deref(), m.admin_env_key.as_deref()]
                .into_iter()
                .flatten()
                .any(|name| std::env::var(name).is_ok_and(|value| !value.trim().is_empty()))
        });
    if has_env_credential {
        return CredentialStatus::Environment;
    }
    if presence.has_application_key || presence.has_admin_key {
        return CredentialStatus::Configured;
    }
    let has_auth_helper = desc
        .routes
        .iter()
        .any(|r| r.credential_route == CredentialRoute::AuthHelper);
    if has_auth_helper {
        return CredentialStatus::Helper;
    }
    if desc
        .routes
        .iter()
        .any(|r| r.credential_route == CredentialRoute::None)
    {
        return CredentialStatus::Unavailable;
    }
    let expects_credential = desc
        .routes
        .iter()
        .any(|r| r.credential_route != CredentialRoute::None);
    if expects_credential {
        return CredentialStatus::Missing;
    }
    CredentialStatus::Unavailable
}

/// Map an authoritative cache read error to a bounded validity label.
fn cache_validity(err: &crate::provider_registry::CacheValidationError) -> CacheValidity {
    use crate::provider_registry::CacheValidationError;
    match err {
        CacheValidationError::Tombstoned => CacheValidity::Tombstoned,
        CacheValidationError::Corrupt(_) => CacheValidity::Corrupt,
        CacheValidationError::VersionMismatch { .. }
        | CacheValidationError::ProviderMismatch { .. }
        | CacheValidationError::OriginMismatch
        | CacheValidationError::KindMismatch
        | CacheValidationError::SurfaceMismatch
        | CacheValidationError::RouteMismatch
        | CacheValidationError::IncarnationMismatch
        | CacheValidationError::BindingMismatch
        | CacheValidationError::OrgProjectMismatch
        | CacheValidationError::BaselineMismatch { .. } => CacheValidity::Mismatch,
        CacheValidationError::Io(_) => CacheValidity::Unavailable,
    }
}

fn build_cache_capability(
    home: &Path,
    id: &str,
    desc: &crate::provider_registry::ProviderInstanceDescriptor,
) -> (
    Option<CacheSummaryInspect>,
    Option<CapabilitySummaryInspect>,
) {
    use crate::provider_registry::{
        CacheOrigin, CapabilityCacheStore, CatalogCacheStore, ProviderCacheIdentity,
        ProviderCacheStore, ProviderId, normalize_endpoint_origin,
    };
    let Some(pid) = ProviderId::new(id).ok() else {
        return (None, None);
    };
    let Some(origin) = desc
        .base_url
        .as_deref()
        .and_then(|b| normalize_endpoint_origin(b).ok())
    else {
        // No per-instance base URL -> nothing to validate against; report
        // not-checked so the boundary between "absent" and "unchecked" is visible.
        return (
            Some(CacheSummaryInspect {
                validity: CacheValidity::NotChecked,
                model_count: None,
                fetched_label: None,
                catalog_generation: None,
            }),
            None,
        );
    };
    // Full authoritative validation is possible only when the current
    // descriptor has one exact route/incarnation and the state supplies the
    // opaque binding/fingerprint. Those values are used for comparison only
    // and never serialized.
    let (identity, state_error) = match ProviderCacheStore::load_state(home, &pid) {
        Ok(Some(state)) => {
            let current_route = desc.primary_route().filter(|route| desc.routes.len() == 1);
            let identity = match (desc.incarnation.clone(), current_route) {
                (Some(incarnation), Some(route)) => ProviderCacheIdentity::new(
                    pid.clone(),
                    incarnation,
                    desc.kind,
                    route.api_surface,
                    route.credential_route,
                    origin.clone(),
                    state.org_project_fingerprint,
                    state.credential_binding_id,
                )
                .ok(),
                _ => None,
            };
            (identity, None)
        }
        Ok(None) => (None, None),
        Err(error) => (None, Some(cache_validity(&error))),
    };

    let catalog = if let Some(identity) = identity.as_ref() {
        match ProviderCacheStore::load_catalog(home, identity) {
            Ok(Some(entry)) => {
                let fetched_label = match entry.origin {
                    CacheOrigin::Live => "live",
                    CacheOrigin::Manual | CacheOrigin::Probe => "cache",
                    CacheOrigin::LegacyMigration => "legacy",
                }
                .to_owned();
                Some(CacheSummaryInspect {
                    validity: CacheValidity::Valid,
                    model_count: Some(entry.models.len()),
                    fetched_label: Some(fetched_label),
                    catalog_generation: Some(entry.catalog_generation),
                })
            }
            Ok(None) => Some(CacheSummaryInspect {
                validity: CacheValidity::NotChecked,
                model_count: None,
                fetched_label: None,
                catalog_generation: None,
            }),
            Err(error) => Some(CacheSummaryInspect {
                validity: cache_validity(&error),
                model_count: None,
                fetched_label: None,
                catalog_generation: None,
            }),
        }
    } else {
        match CatalogCacheStore::load(home, &pid, &origin) {
            Ok(_) => Some(CacheSummaryInspect {
                validity: CacheValidity::NotChecked,
                model_count: None,
                fetched_label: None,
                catalog_generation: None,
            }),
            Err(error) => Some(CacheSummaryInspect {
                validity: cache_validity(&error),
                model_count: None,
                fetched_label: None,
                catalog_generation: None,
            }),
        }
    };
    // A one-shot inspect cannot derive the live capability baseline. The
    // impossible sentinel deliberately maps any otherwise-valid envelope to
    // `NotChecked`; corruption, tombstones, and identity failures stay visible.
    let capability = if let Some(validity) = state_error {
        Some(CapabilitySummaryInspect {
            validity,
            fetched_label: None,
        })
    } else {
        match CapabilityCacheStore::load(home, &pid, &origin, "__inspect_unavailable_baseline__") {
            Ok(Some(_)) => Some(CapabilitySummaryInspect {
                validity: CacheValidity::NotChecked,
                fetched_label: None,
            }),
            Ok(None) => None,
            Err(crate::provider_registry::CacheValidationError::BaselineMismatch { .. }) => {
                Some(CapabilitySummaryInspect {
                    validity: CacheValidity::NotChecked,
                    fetched_label: None,
                })
            }
            Err(error) => Some(CapabilitySummaryInspect {
                validity: cache_validity(&error),
                fetched_label: None,
            }),
        }
    };
    (catalog, capability)
}

fn build_references(
    mgmt: &crate::provider_registry::ProviderManagementService,
    id: &str,
) -> Option<ReferenceInspect> {
    let snap = mgmt.reference_impact(id).ok()?;
    let groups: Vec<ReferenceGroupInspect> = snap
        .groups
        .iter()
        .map(|g| ReferenceGroupInspect {
            kind: g.kind.label().to_owned(),
            count: g.references.len(),
        })
        .collect();
    Some(ReferenceInspect {
        can_remove: snap.can_remove,
        blocked_reason: snap.blocked_reason.as_deref().map(bound),
        groups,
        truncated: Some(snap.truncated),
    })
}

fn catalog_origin_str(origin: crate::agent::model_identity::CatalogEntryOrigin) -> &'static str {
    use crate::agent::model_identity::CatalogEntryOrigin;
    match origin {
        CatalogEntryOrigin::LegacyBuiltIn => "legacy_builtin",
        CatalogEntryOrigin::ExplicitUser => "explicit_user",
        CatalogEntryOrigin::GeneratedBuiltIn => "generated_builtin",
        CatalogEntryOrigin::GeneratedAdditionalAccount => "generated_additional_account",
    }
}

fn provider_kind_label(kind: crate::agent::model_providers::ModelProviderKind) -> &'static str {
    use crate::agent::model_providers::ModelProviderKind;
    match kind {
        ModelProviderKind::Xai => "xai",
        ModelProviderKind::OpenAi => "openai",
        ModelProviderKind::OpenRouter => "openrouter",
        ModelProviderKind::Anthropic => "anthropic",
        ModelProviderKind::Zai => "zai",
        ModelProviderKind::OpenAiCompatible => "openai_compatible",
    }
}

/// Alias inputs that are safe identifiers (upstream slug and explicit id).
/// Human display names are excluded to honor the no-arbitrary-display-name rule.
fn alias_inputs(entry: &crate::agent::config::ModelEntry) -> Vec<String> {
    let mut v = Vec::new();
    v.push(entry.info.model.clone());
    if let Some(id) = &entry.info.id {
        v.push(id.clone());
    }
    v
}

fn classify_alias(
    catalog: &IndexMap<String, crate::agent::config::ModelEntry>,
    origins: &crate::agent::model_identity::CatalogOrigins,
    input: &str,
) -> AliasRecordInspect {
    use crate::agent::model_identity::{
        ModelIdentityProvenance, ModelIdentityResolution, resolve_model_identity_with_origins,
    };
    match resolve_model_identity_with_origins(catalog, origins, input) {
        ModelIdentityResolution::Resolved(resolved) => AliasRecordInspect {
            input: input.to_owned(),
            kind: match resolved.provenance {
                ModelIdentityProvenance::ExactCanonical => "exact".to_owned(),
                ModelIdentityProvenance::PermanentCompatibility => {
                    "permanentCompatibility".to_owned()
                }
                ModelIdentityProvenance::UniqueLegacyAlias => "uniqueLegacy".to_owned(),
            },
            canonical_id: Some(resolved.canonical_id.as_str().to_owned()),
            candidates: Vec::new(),
        },
        ModelIdentityResolution::Ambiguous { candidates, .. } => {
            let ids: Vec<String> = candidates.iter().map(|c| c.as_str().to_owned()).collect();
            AliasRecordInspect {
                input: input.to_owned(),
                kind: "ambiguous".to_owned(),
                canonical_id: None,
                candidates: ids,
            }
        }
        ModelIdentityResolution::Missing { .. } => AliasRecordInspect {
            input: input.to_owned(),
            kind: "missing".to_owned(),
            canonical_id: None,
            candidates: Vec::new(),
        },
    }
}

fn insert_gated_aliases(
    aliases: &mut std::collections::BTreeMap<String, AliasRecordInspect>,
    gated_entries: &IndexMap<String, crate::agent::config::ModelEntry>,
) {
    // The rollout gate removes additional-account rows from the published
    // catalog. Add only safe identifier aliases that are absent from the live
    // catalog so a gated entry is never mislabeled as genuinely missing.
    for (key, entry) in gated_entries {
        for input in alias_inputs(entry) {
            aliases
                .entry(input.clone())
                .or_insert_with(|| AliasRecordInspect {
                    input,
                    kind: "gated".to_owned(),
                    canonical_id: Some(key.clone()),
                    candidates: Vec::new(),
                });
        }
    }
}

fn build_model_catalog(home: &Path) -> ModelCatalogInspect {
    let effective_config = crate::config::load_effective_config();
    let Some(cfg) = effective_config
        .as_ref()
        .ok()
        .and_then(|ec| crate::agent::config::Config::new_from_toml_cfg(ec).ok())
    else {
        return ModelCatalogInspect {
            generation: 0,
            total_visible_count: 0,
            auth_view: "api_key".to_owned(),
            models_cache: "unavailable".to_owned(),
            default: None,
            models: Vec::new(),
            duplicate_groups: Vec::new(),
            aliases: Vec::new(),
            gated_entries: Vec::new(),
            warnings: Vec::new(),
            diagnostic: Some("invalid config".to_owned()),
        };
    };
    model_catalog_from_cfg(&cfg, home)
}

/// Build the catalog from an already-parsed config. Split out so tests can
/// pass a controlled `Config` without touching the process `GROK_HOME`.
fn model_catalog_from_cfg(cfg: &crate::agent::config::Config, home: &Path) -> ModelCatalogInspect {
    let result = crate::agent::models::inspect_catalog_for_home(home, cfg);
    let catalog = &result.catalog;
    let origins = &result.origins;

    let warnings: Vec<String> = cfg
        .config_warnings
        .iter()
        .take(MAX_INSPECT_WARNINGS)
        .map(|w| bound(&format!("[{}] {}", w.target.label(), w.reason)))
        .collect();

    // Overlapping upstream slugs -> one group, canonical candidates sorted.
    let mut by_upstream: IndexMap<String, Vec<String>> = IndexMap::new();
    for (key, entry) in catalog {
        by_upstream
            .entry(entry.info.model.clone())
            .or_default()
            .push(key.clone());
    }
    let mut duplicate_groups = Vec::new();
    for (upstream_id, mut ids) in by_upstream {
        if ids.len() > 1 {
            ids.sort();
            duplicate_groups.push(DuplicateGroupInspect {
                upstream_id,
                canonical_ids: ids,
            });
        }
    }

    let models: Vec<ModelRowInspect> = catalog
        .iter()
        .map(|(key, entry)| ModelRowInspect {
            canonical_id: key.clone(),
            upstream_id: entry.info.model.clone(),
            provider_instance_id: entry.model_provider.as_ref().map(|p| p.id.clone()),
            kind: entry
                .model_provider
                .as_ref()
                .map(|p| provider_kind_label(p.kind).to_owned()),
            origin: catalog_origin_str(crate::agent::model_identity::origin_for_key(origins, key))
                .to_owned(),
            user_selectable: entry.info.user_selectable,
            hidden: entry.info.hidden,
        })
        .collect();

    let (default_key, default_entry, default_source) =
        crate::agent::models::resolve_default_model_with_origins(cfg, catalog, origins, false);
    let default = Some(ModelDefaultInspect {
        id: default_key,
        source: default_source.to_string(),
        upstream: default_entry.info.model,
    });

    // Alias enumeration sorted by input via a BTreeMap key.
    let mut alias_by_input: std::collections::BTreeMap<String, AliasRecordInspect> =
        std::collections::BTreeMap::new();
    let mut record_alias = |input: &str| {
        if alias_by_input.contains_key(input) {
            return;
        }
        let record = classify_alias(catalog, origins, input);
        alias_by_input.insert(input.to_owned(), record);
    };
    for (key, entry) in catalog {
        for input in alias_inputs(entry) {
            if input != *key {
                record_alias(&input);
            }
        }
        if crate::agent::model_identity::is_reserved_catalog_key(key) {
            record_alias(key);
        }
    }
    insert_gated_aliases(&mut alias_by_input, &result.gated_additional_entries);
    let aliases: Vec<AliasRecordInspect> = alias_by_input.into_values().collect();

    let mut gated_entries = Vec::new();
    if !crate::provider_registry::multi_account_rollout_enabled() {
        for (key, origin) in origins {
            if origin.is_generated_additional_account() {
                gated_entries.push(key.clone());
                if gated_entries.len() >= MAX_NAMES {
                    break;
                }
            }
        }
    }

    ModelCatalogInspect {
        generation: 0,
        total_visible_count: result.total_visible_count,
        auth_view: "api_key".to_owned(),
        models_cache: result.cache_status.to_owned(),
        default,
        models,
        duplicate_groups,
        aliases,
        gated_entries,
        warnings,
        diagnostic: None,
    }
}

fn prime_from_skill(cfg: &xai_grok_config_types::SkillPrimeConfig) -> PrimeSectionInspect {
    PrimeSectionInspect {
        enabled: cfg.enabled,
        retrieval_profile: cfg.retrieval_profile.clone(),
        max_results: Some(cfg.max_results),
        max_tokens: Some(cfg.max_tokens),
        max_total_chars: Some(cfg.max_total_chars),
        max_context_fraction: Some(cfg.max_context_fraction),
        deadline_ms: Some(cfg.deadline_ms),
        degrade_on_error: cfg.degrade_on_error,
    }
}

fn prime_from_agent(cfg: &xai_grok_config_types::AgentPrimeConfig) -> PrimeSectionInspect {
    PrimeSectionInspect {
        enabled: cfg.enabled,
        retrieval_profile: cfg.retrieval_profile.clone(),
        max_results: Some(cfg.max_results),
        max_tokens: Some(cfg.max_tokens),
        max_total_chars: Some(cfg.max_total_chars),
        max_context_fraction: Some(cfg.max_context_fraction),
        deadline_ms: Some(cfg.deadline_ms),
        degrade_on_error: cfg.degrade_on_error,
    }
}

/// Pure verdict for the published-vs-disk boundary. Unit-tested.
fn retrieval_verdict(disk_ok: bool, published_enabled: bool) -> (RetrievalValidity, bool) {
    if !disk_ok {
        (RetrievalValidity::Invalid, published_enabled)
    } else if published_enabled {
        (RetrievalValidity::Valid, false)
    } else {
        (RetrievalValidity::Disabled, false)
    }
}

fn build_retrieval(home: &Path, cwd: Option<&Path>) -> RetrievalInspect {
    use crate::retrieval::registry::RetrievalRegistry;
    use crate::retrieval::reload::{build_snapshot, load_build_input_from_home};

    // Prefer an installed live registry when inspect is embedded in a running
    // process. A one-shot inspect has no live LKG; it builds an isolated disk
    // view and labels the source accordingly.
    let live_registry = crate::retrieval::registry_for_home(home);
    let registry = live_registry
        .clone()
        .unwrap_or_else(|| RetrievalRegistry::load_from_home(home));
    let snap = registry.load();

    // Validate the current disk candidate without publishing it. Never expose
    // raw parse/IO errors because TOML diagnostics can contain source lines.
    let (disk_ok, disk_diag) = match load_build_input_from_home(home) {
        Ok(input) => match build_snapshot(input, snap.generation) {
            Ok(_) => (true, None),
            Err(err) => (false, Some(bound_reasons(err.reasons))),
        },
        Err(error) => {
            let label = if error.starts_with("config.toml parse error:") {
                "config_parse_error"
            } else if error.starts_with("read config.toml:") {
                "config_read_error"
            } else {
                "retrieval_unavailable"
            };
            (false, Some(label.to_owned()))
        }
    };
    let (validity, lkg) = if live_registry.is_some() {
        retrieval_verdict(disk_ok, snap.enabled)
    } else if disk_ok {
        retrieval_verdict(true, snap.enabled)
    } else {
        // No live process exists to retain LKG, but the disk candidate is
        // still invalid rather than deliberately disabled.
        (RetrievalValidity::Invalid, false)
    };

    let embedding_models: Vec<EmbeddingRouteInspect> = snap
        .embedding_models
        .iter()
        .map(|(id, r)| EmbeddingRouteInspect {
            id: id.clone(),
            provider_instance_id: r.provider_instance_id.clone(),
            incarnation: r.incarnation.clone(),
            model: r.config.model.clone(),
            protocol: r.config.protocol.as_str().to_owned(),
            dimensions: r.config.dimensions,
            encoding: r.config.encoding.as_str().to_owned(),
            timeout_ms: r.request_timeout_ms,
        })
        .collect();

    let reranker_models: Vec<RerankerRouteInspect> = snap
        .reranker_models
        .iter()
        .map(|(id, r)| RerankerRouteInspect {
            id: id.clone(),
            provider_instance_id: r.provider_instance_id.clone(),
            incarnation: r.incarnation.clone(),
            model: r.config.model.clone(),
            protocol: r.config.protocol.as_str().to_owned(),
            timeout_ms: r.request_timeout_ms,
        })
        .collect();

    let profiles: Vec<ProfileInspect> = snap
        .profiles
        .iter()
        .map(|(id, p)| ProfileInspect {
            id: id.clone(),
            embedding_route_ids: p.embedding_route_ids.clone(),
            reranker_route_ids: p.reranker_route_ids.clone(),
            budgets: ProfileBudgetsInspect {
                deadline_ms: p.budgets.deadline.as_millis() as u64,
                max_attempts: p.budgets.max_attempts,
                max_input_tokens: p.budgets.max_input_tokens,
                max_output_tokens: p.budgets.max_output_tokens,
                max_candidates: p.budgets.max_candidates,
                max_results: p.budgets.max_results,
            },
            fallback_strategy: p.fallback_strategy.as_str().to_owned(),
        })
        .collect();

    let prime = PrimeConfigInspect {
        skills: prime_from_skill(&snap.prime.skills),
        agents: prime_from_agent(&snap.prime.agents),
    };

    let memory_pin = snap.memory_retrieval_profile.as_deref().and_then(|pid| {
        let profile = snap.profiles.get(pid)?;
        let emb_id = profile.embedding_route_ids.first()?;
        let route = snap.embedding_models.get(emb_id)?;
        let cwd = cwd?;
        let storage = crate::session::memory::MemoryStorage::new(cwd, Some(&home.join("memory")));
        let db_path = storage.workspace_dir().join("index.sqlite");
        let conn = xai_sqlite_journal::JournalMode::for_db_path(&db_path)
            .open_readonly(&db_path)
            .ok()?;
        let installed_hash: String = conn
            .query_row(
                "SELECT value FROM meta
                 WHERE key = 'vector_fingerprint_hash'
                   AND length(CAST(value AS BLOB)) = ?1",
                [MEMORY_FINGERPRINT_HASH_LEN as i64],
                |row| row.get(0),
            )
            .ok()?;
        let installed_payload: String = conn
            .query_row(
                "SELECT value FROM meta
                 WHERE key = 'vector_fingerprint'
                   AND length(CAST(value AS BLOB)) BETWEEN 1 AND ?1",
                [MAX_MEMORY_FINGERPRINT_PAYLOAD_BYTES as i64],
                |row| row.get(0),
            )
            .ok()?;
        let rebuild_pending: bool = conn
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM meta
                     WHERE key = 'vector_rebuild_pending' AND trim(value) <> ''
                 )",
                [],
                |row| row.get(0),
            )
            .ok()?;
        if !installed_hash
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            || rebuild_pending
        {
            return None;
        }
        let installed: InstalledMemoryFingerprint =
            serde_json::from_str(&installed_payload).ok()?;
        let safe_installed_labels = [
            installed.source.provider_instance_id.as_str(),
            installed.source.model.as_str(),
            installed.source.protocol.as_str(),
        ]
        .into_iter()
        .all(|value| {
            !value.is_empty()
                && value.chars().count() <= MAX_DIAG_LEN
                && !value.chars().any(|ch| (ch as u32) < 0x20)
        });
        if !safe_installed_labels {
            return None;
        }
        let installed_incarnation = installed
            .source
            .incarnation
            .as_deref()
            .filter(|value| *value != "null");
        let configured_route_matches_installed = route.provider_instance_id
            == installed.source.provider_instance_id
            && route.incarnation.as_deref() == installed_incarnation
            && route.origin_host == installed.source.origin_host
            && xai_grok_inference::DEFAULT_EMBEDDINGS_PATH == installed.source.embedding_path
            && route.config.model == installed.source.model
            && route.config.protocol.as_str() == installed.source.protocol
            && route.config.dimensions.map(|value| value as usize)
                == Some(installed.source.dimensions)
            && route.config.encoding.as_str() == installed.source.encoding;
        Some(MemoryPinInspect {
            configured_profile: pid.to_owned(),
            installed_provider_instance_id: installed.source.provider_instance_id,
            installed_model: installed.source.model,
            installed_protocol: installed.source.protocol,
            configured_route_matches_installed,
            embedding_space_fingerprint_short: installed_hash.chars().take(12).collect(),
            pinned_until_rebuild_or_new_session: true,
        })
    });

    let warnings: Vec<String> =
        bound_warns(snap.warnings.iter().map(String::as_str).collect::<Vec<_>>());

    RetrievalInspect {
        source: if live_registry.is_some() {
            "published".to_owned()
        } else {
            "disk".to_owned()
        },
        validity,
        generation: snap.generation,
        graph_generation: snap.graph_generation,
        provider_generation: snap.provider_generation,
        enabled: snap.enabled,
        fingerprint: Some(inspect_fingerprint_short(&snap.fingerprint)),
        embedding_models,
        reranker_models,
        profiles,
        prime,
        memory_retrieval_profile: snap.memory_retrieval_profile.clone(),
        memory_pin,
        warnings,
        last_known_good_retained: if lkg { Some(true) } else { None },
        diagnostic: disk_diag,
        prime_index: crate::session::prime::inspect_status(home, cwd).map(|s| PrimeIndexInspect {
            generation: s.generation,
            fingerprint_short: s.fingerprint_short,
            skills_items: s.skills.item_count,
            skills_vectors: s.skills.vector_count,
            skills_readiness: s.skills.readiness,
            agents_items: s.agents.item_count,
            agents_vectors: s.agents.vector_count,
            agents_readiness: s.agents.readiness,
            job_state: s.job.map(|j| j.state),
            configured_route: crate::session::prime::displayable_configured_route(
                s.configured_route.as_deref(),
            )
            .map(str::to_owned),
        }),
        vector_mirrors: crate::session::vector_mirror::registered_mirrors()
            .iter()
            .map(|handle| {
                let snapshot = handle.snapshot();
                VectorMirrorInspect {
                    backend: handle.mirror().backend_id().to_owned(),
                    collection: handle.collection().to_owned(),
                    state: match snapshot.state {
                        xai_grok_memory::MirrorState::Syncing => "syncing",
                        xai_grok_memory::MirrorState::Ready => "ready",
                        xai_grok_memory::MirrorState::Unavailable => "unavailable",
                        xai_grok_memory::MirrorState::Unconfigured => "unconfigured",
                    }
                    .to_owned(),
                    row_count: snapshot.row_count,
                }
            })
            .collect(),
    }
}

async fn build_report(cwd: &Path) -> InspectReport {
    let effective_config_result = crate::config::load_effective_config();
    let effective_config = effective_config_result
        .as_ref()
        .cloned()
        .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));
    // Parse compatibility separately so malformed cells cannot block unrelated sections.
    let mut config_without_compat = effective_config.clone();
    if let Some(table) = config_without_compat.as_table_mut() {
        table.remove("compat");
    }
    let parsed_config =
        crate::agent::config::Config::new_from_toml_cfg(&config_without_compat).ok();

    let git_root = git2::Repository::discover(cwd)
        .ok()
        .and_then(|r| r.workdir().map(|p| p.to_path_buf()));

    // Route through the live folder-trust gate rather than a raw store read; no
    // session resolve has run for a one-shot `inspect`. The single verdict drives
    // the top-level flag and gates the hooks, plugins, and MCP/LSP listings so
    // they reflect runtime gating. `remote = None`: env/user/managed opt-out is
    // honored, but a remote kill-switch is not consulted on this report-only path.
    crate::agent::folder_trust::resolve_and_record(cwd, None, false);
    let project_trusted = crate::agent::folder_trust::project_scope_allowed(cwd);

    let trust_store = xai_grok_agent::plugins::TrustStore::load();
    let mut plugins_cfg: crate::agent::config::PluginsConfig = effective_config
        .get("plugins")
        .and_then(|v| v.clone().try_into().ok())
        .unwrap_or_default();
    plugins_cfg.merge_claude_enabled_plugins(Some(cwd));
    let mut plugin_config = plugins_cfg.to_discovery_config();
    // Project plugins gate on the same folder-trust verdict as hooks and the live
    // session/doctor sites, so the listing's `enabled` flags match runtime gating.
    let discovered_plugins = xai_grok_agent::plugins::discover_plugins(
        Some(cwd),
        &plugin_config,
        &trust_store,
        project_trusted,
    );
    plugin_config.populate_plugin_lists(&discovered_plugins);

    let plugin_registry = xai_grok_agent::plugins::PluginRegistry::from_discovered(
        discovered_plugins.clone(),
        &plugin_config.disabled,
        &plugin_config.enabled,
    );

    let external_compat = resolve_inspect_compat(effective_config_result.as_ref().map_err(|_| ()));

    // Same `[skills]` table the runtime loads, so `paths` skills appear,
    // `ignore`d ones are hidden, and `disabled` ones surface as disabled.
    let skills_config = crate::config::parse_skills_config(&effective_config);

    // Discover with all vendors ON so inspect shows the full set on disk.
    let (mut instructions, permissions, mut skills) = tokio::join!(
        list_instructions(cwd),
        list_permissions(cwd, project_trusted),
        list_skills(cwd, &plugin_registry, &skills_config),
    );

    // Attach local compatibility status to each discovered vendor entry.
    for entry in &mut instructions {
        entry.compatibility_status =
            instruction_compat_status(&entry.vendor, &entry.file_type, &external_compat);
        entry.disabled |= entry.compatibility_status == Some(CompatEntryStatus::Disabled);
    }
    for entry in &mut skills {
        entry.compatibility_status =
            vendor_compat_status(&entry.vendor, "skills", &external_compat);
        entry.disabled |= entry.compatibility_status == Some(CompatEntryStatus::Disabled);
    }
    let mut hooks = list_hooks(git_root.as_deref(), project_trusted, &discovered_plugins);
    for entry in &mut hooks {
        entry.compatibility_status = vendor_compat_status(&entry.vendor, "hooks", &external_compat);
        entry.disabled |= entry.compatibility_status == Some(CompatEntryStatus::Disabled);
    }
    let agents = list_agents(cwd, &plugin_registry);
    let plugins = list_plugins(&discovered_plugins);
    let marketplaces = list_marketplaces(git_root.as_deref());
    let mut mcp = list_mcp_servers(cwd, &plugin_registry);
    for entry in &mut mcp {
        entry.compatibility_status = vendor_compat_status(&entry.vendor, "mcps", &external_compat);
        entry.disabled |= entry.compatibility_status == Some(CompatEntryStatus::Disabled);
    }
    let lsp = list_lsp_servers(cwd, &discovered_plugins);
    let configs = list_config_sources(cwd);
    let config_warnings = parsed_config
        .as_ref()
        .map(|c| c.config_warnings.clone())
        .unwrap_or_default();

    let home = crate::util::grok_home::grok_home();
    let provider_registry = build_provider_registry(&home);
    let model_catalog = build_model_catalog(&home);
    let retrieval = build_retrieval(&home, Some(cwd));

    InspectReport {
        grok_version: xai_grok_version::VERSION.to_string(),
        channel: crate::util::config::channel_name_from_cache()
            .unwrap_or("unknown")
            .to_string(),
        cwd: workspace_storage_identity(cwd),
        project_root: git_root.map(|_| workspace_storage_identity(cwd)),
        project_trusted,
        project_instructions: instructions,
        permissions,
        login_policy: login_policy_report(parsed_config.as_ref()),
        hooks,
        skills,
        agents,
        plugins,
        marketplaces,
        mcp_servers: mcp,
        lsp_servers: lsp,
        config_sources: configs,
        external_compat,
        config_warnings,
        provider_registry: Some(provider_registry),
        model_catalog: Some(model_catalog),
        retrieval: Some(retrieval),
    }
}

/// Read `[paths] extra_rule_dirs` from the effective config. Returns empty
/// on any read/parse failure so misconfiguration never breaks classification.
fn extra_rule_dirs_from_config() -> Vec<String> {
    let Ok(root) = crate::config::load_effective_config() else {
        return Vec::new();
    };
    root.get("paths")
        .and_then(|v| v.get("extra_rule_dirs"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn has_rules_directory(file_path: &str, config_dir: &str) -> bool {
    let mut previous = None;
    for component in file_path
        .split(['/', '\\'])
        .filter(|component| !component.is_empty())
    {
        if previous == Some(config_dir) && component == "rules" {
            return true;
        }
        previous = Some(component);
    }
    false
}

fn instruction_scope(
    file_path: &str,
    grok_home: &Path,
    vendor_homes: &[(PathBuf, bool)],
    workspace_root: &Path,
) -> Scope {
    if crate::util::is_user_instruction_path(
        Path::new(file_path),
        grok_home,
        vendor_homes,
        Some(workspace_root),
    ) {
        Scope::Global
    } else {
        Scope::Project
    }
}

fn instruction_file_type(
    file_path: &str,
    grok_home: &Path,
    claude_imported: bool,
    extra_rule_prefixes: &[PathBuf],
) -> &'static str {
    let path = Path::new(file_path);
    if path
        .parent()
        .is_some_and(|parent| parent == grok_home.join("rules"))
        || has_rules_directory(file_path, ".grok")
        || has_rules_directory(file_path, ".cursor")
        || (!claude_imported && has_rules_directory(file_path, ".claude"))
        || extra_rule_prefixes
            .iter()
            .any(|prefix| path.starts_with(prefix))
    {
        "rules"
    } else {
        "agents_md"
    }
}

/// Wraps the production instruction discovery (`agents_md::read_agents_config_with_paths`).
async fn list_instructions(cwd: &Path) -> Vec<InstructionFile> {
    // Discover with all vendors ON so inspect shows the full set.
    let configs = xai_grok_agent::prompt::agents_md::read_agents_config_with_paths(
        &cwd.display().to_string(),
        xai_grok_agent::prompt::skills::CompatConfig::default(),
    )
    .await;

    let grok_home = crate::util::grok_home::grok_home();
    let vendor_homes = dirs::home_dir()
        .map(|home_dir| {
            vec![
                (home_dir.join(".claude"), true),
                (home_dir.join(".cursor"), true),
            ]
        })
        .unwrap_or_default();
    let workspace_root = git2::Repository::discover(cwd)
        .ok()
        .and_then(|repo| repo.workdir().map(Path::to_path_buf))
        .unwrap_or_else(|| cwd.to_path_buf());

    // Phase 2 cutoff: when imported, stop classifying `.claude/rules/` paths
    // as rules. Equivalent dirs come in via `[paths] extra_rule_dirs`.
    let imported = crate::claude_import::is_claude_import_marked();
    let extra_rule_dirs = extra_rule_dirs_from_config();
    // Pre-expand `~/` and resolve once, so the per-config-file matching loop
    // can use a clean prefix check. Empty/invalid paths fall
    // through to a no-op match.
    //
    // TODO(phase-3): `extra_rule_dirs` only re-classifies files that
    // `xai_grok_agent::prompt::agents_md::read_agents_config_with_paths`
    // has already discovered. Plumbing `extra_rule_dirs` through to that
    // discovery (so files in arbitrary user-configured dirs are surfaced as
    // rules instead of being missed entirely) is out of scope for this stack
    // (intentional wontfix for now).
    // Skills (`extensions/skills.rs`) take the typed-scan path so they don't
    // have this limitation; rules need the same treatment in a follow-up.
    let extra_rule_prefixes: Vec<std::path::PathBuf> = extra_rule_dirs
        .iter()
        .map(|d| crate::claude_import::expand_home(d))
        .collect();

    configs
        .into_iter()
        .map(|c| {
            let file_type =
                instruction_file_type(&c.file_path, &grok_home, imported, &extra_rule_prefixes);
            let scope = instruction_scope(&c.file_path, &grok_home, &vendor_homes, &workspace_root);
            let size = c.content.len();
            let vendor = derive_vendor(&c.file_path).map(String::from);
            InstructionFile {
                size_bytes: size,
                approx_tokens: estimate_tokens(&c.content),
                path: c.file_path,
                scope,
                file_type: file_type.to_string(),
                vendor,
                disabled: false,
                compatibility_status: None,
            }
        })
        .collect()
}

/// Calls the production permission resolver (`resolve_permissions_with_provenance`)
/// which handles both Grok TOML and vendor settings fallback in one codepath.
async fn list_permissions(cwd: &Path, project_trusted: bool) -> PermissionsReport {
    use xai_grok_workspace::permission::resolution;

    let ms = resolution::managed_settings();
    let format_entry = |e: &resolution::AllowedMcpServer| match e {
        resolution::AllowedMcpServer::Http { url_pattern } => url_pattern.clone(),
        resolution::AllowedMcpServer::Stdio { command } => format!("command:{command}"),
        resolution::AllowedMcpServer::Name { name } => format!("name:{name}"),
    };
    let mcp_server_allowlist: Vec<String> = ms
        .mcp_allowlist
        .entries
        .iter()
        .map(format_entry)
        .chain(
            ms.mcp_allowlist
                .deny_entries
                .iter()
                .map(|e| format!("deny:{}", format_entry(e))),
        )
        .collect();
    let marketplace_allowlist = ms.marketplace_allowlist.allowed_urls.clone();

    // Managed settings presence + enforced policy computed unconditionally (before
    // the early return) so that a managed-settings.json containing *only* e.g.
    // disableBypassPermissionsMode still surfaces its path and effects.
    let managed_settings_path =
        crate::config::claude_managed_settings_probe_path().map(|p| p.display().to_string());
    let managed_settings_exists =
        crate::config::claude_managed_settings_probe_path().is_some_and(|p| p.exists());
    // `source_path` is set only on the successful read+parse path, so it is the
    // signal for "actually loaded" (vs present-but-broken).
    let managed_settings_active = ms.features.source_path.is_some();

    let mut enforced = Vec::new();
    if let Some(src) = &ms.features.source_path {
        let source = src
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "managed-settings.json".to_string());
        for (flag, setting) in [
            (ms.features.disable_yolo, "alwaysApprove"),
            (ms.features.disable_telemetry, "telemetry"),
            (ms.features.disable_feedback, "feedback"),
        ] {
            if flag == Some(true) {
                enforced.push(EnforcedPolicy {
                    setting: setting.to_string(),
                    enabled: false,
                    source: source.clone(),
                });
            }
        }
    }

    let Some(resolved) =
        resolution::resolve_permissions_with_provenance(cwd, project_trusted).await
    else {
        return PermissionsReport {
            sources: vec![],
            loaded: 0,
            skipped: vec![],
            mcp_server_allowlist,
            marketplace_allowlist,
            managed_settings_path: managed_settings_path.clone(),
            managed_settings_exists,
            managed_settings_active,
            enforced: enforced.clone(),
        };
    };

    let mut sources: Vec<String> = resolved
        .sources
        .iter()
        .map(inspect_requirement_source_label)
        .collect();
    sources.dedup();

    let skipped = resolved
        .skipped
        .into_iter()
        .map(|s| SkippedRule {
            rule: s.rule,
            reason: s.reason,
        })
        .collect();

    PermissionsReport {
        sources,
        loaded: resolved.config.rules.len(),
        skipped,
        mcp_server_allowlist,
        marketplace_allowlist,
        managed_settings_path,
        managed_settings_exists,
        managed_settings_active,
        enforced,
    }
}

/// Resolves the enterprise login-hardening knobs from the merged config
/// (`[grok_com_config]`, the `[auth]` alias, and env overrides) so admins can
/// confirm the deployment's auth policy actually loaded.
fn login_policy_report(config: Option<&crate::agent::config::Config>) -> LoginPolicyReport {
    let grok_com_config = config
        .map(|c| c.grok_com_config.clone())
        .unwrap_or_default();
    LoginPolicyReport {
        api_key_auth_disabled: grok_com_config.api_key_auth_disabled(),
        disable_api_key_auth: grok_com_config.disable_api_key_auth,
        force_login_team_uuid: grok_com_config.force_login_team_uuid,
    }
}

/// Discovers hooks with every vendor enabled so compatibility can be annotated later.
fn list_hooks(
    git_root: Option<&Path>,
    project_trusted: bool,
    discovered_plugins: &[xai_grok_agent::plugins::DiscoveredPlugin],
) -> Vec<HookEntry> {
    let all_on = xai_grok_tools::types::compat::CompatConfig::default();
    let source_paths = crate::util::hooks::discover_hook_source_paths(git_root, &all_on);
    let (global_sources, project_sources) = source_paths.as_sources(project_trusted);

    let (registry, _errors) =
        xai_grok_hooks::discovery::load_hooks_from_sources(&global_sources, &project_sources);

    let home_dir = dirs::home_dir();
    let grok_home = xai_grok_config::grok_home();

    let mut entries: Vec<HookEntry> = registry
        .all_hooks()
        .into_iter()
        .map(|h| {
            let is_user_scope = h.source_dir.starts_with(&grok_home)
                || home_dir.as_deref().is_some_and(|home| {
                    h.source_dir.starts_with(home.join(".cursor"))
                        || h.source_dir.starts_with(home.join(".claude"))
                });
            let source = if is_user_scope {
                ConfigSource::User {
                    path: h.source_dir.clone(),
                }
            } else {
                ConfigSource::Project {
                    path: h.source_dir.clone(),
                }
            };
            let vendor = derive_vendor(&h.source_dir.display().to_string()).map(String::from);
            HookEntry {
                event: format!("{:?}", h.event),
                hook_type: h.handler_type.as_str().to_string(),
                target: inspect_target_label(
                    &h.command
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .or_else(|| h.url.clone())
                        .unwrap_or_default(),
                ),
                source,
                matcher: h.configured_matcher.clone(),
                vendor,
                disabled: false,
                compatibility_status: None,
            }
        })
        .collect();

    // Plugin hooks
    for p in discovered_plugins {
        if !p.trusted {
            continue;
        }
        let source = ConfigSource::Plugin {
            plugin_name: p.manifest.name.clone(),
            path: p.root.clone(),
        };
        if let Some(ref hooks_path) = p.hooks_path {
            entries.push(HookEntry {
                event: "(plugin)".to_string(),
                hook_type: "file".to_string(),
                target: inspect_target_label(&hooks_path.display().to_string()),
                source,
                matcher: None,
                vendor: None,
                disabled: false,
                compatibility_status: None,
            });
        } else if p.manifest.inline_hooks().is_some() {
            entries.push(HookEntry {
                event: "(plugin)".to_string(),
                hook_type: "inline".to_string(),
                target: String::new(),
                source,
                matcher: None,
                vendor: None,
                disabled: false,
                compatibility_status: None,
            });
        }
    }

    entries
}

async fn list_skills(
    cwd: &Path,
    plugin_registry: &xai_grok_agent::plugins::PluginRegistry,
    skills_config: &xai_grok_agent::prompt::skills::SkillsConfig,
) -> Vec<SkillEntry> {
    // Discover with all vendors ON so inspect shows the full set.
    let listing = xai_grok_agent::prompt::skills::list_skill_sources_with_plugins(
        Some(&cwd.display().to_string()),
        skills_config,
        Some(plugin_registry),
        xai_grok_agent::prompt::skills::CompatConfig::default(),
    )
    .await;

    let mut entries: Vec<SkillEntry> = listing
        .skills
        .into_iter()
        .map(|s| {
            let source = skill_entry_source(&s);
            let vendor = derive_vendor(&s.path).map(String::from);
            SkillEntry {
                name: s.label().to_string(),
                description: s.description,
                source,
                user_invocable: s.user_invocable,
                vendor,
                // Preserve `[skills].disabled`; compatibility is applied later.
                disabled: !s.enabled,
                compatibility_status: None,
                quarantined: false,
                diagnostic_codes: Vec::new(),
            }
        })
        .collect();
    for row in listing.inventory.quarantined {
        entries.push(SkillEntry {
            name: row.identity.parent_dir_name,
            description: String::new(),
            source: ConfigSource::Project {
                path: PathBuf::from(row.identity.file_label),
            },
            user_invocable: false,
            vendor: None,
            disabled: true,
            compatibility_status: None,
            quarantined: true,
            diagnostic_codes: row
                .diagnostics
                .iter()
                .map(|d| d.code.as_str().to_string())
                .collect(),
        });
    }
    entries
}

/// Resolve the inspect-facing source for a discovered skill.
///
/// Prefers the discovery-stamped `config_source` (plugin skills,
/// `[skills].paths` entries), then falls back to the discovered scope.
fn skill_entry_source(s: &xai_grok_agent::prompt::skills::SkillInfo) -> ConfigSource {
    use xai_grok_tools::implementations::skills::types::SkillScope;

    if let Some(source) = s.config_source.clone() {
        return source;
    }
    let path = PathBuf::from(&s.path);
    match s.scope {
        SkillScope::Local | SkillScope::Repo => ConfigSource::Project { path },
        SkillScope::User => ConfigSource::User { path },
        SkillScope::Server => ConfigSource::Server { path },
        SkillScope::Bundled => ConfigSource::Bundled { path },
        SkillScope::Plugin => ConfigSource::Plugin {
            plugin_name: String::new(),
            path,
        },
    }
}

fn list_agents(
    cwd: &Path,
    plugin_registry: &xai_grok_agent::plugins::PluginRegistry,
) -> Vec<AgentEntry> {
    let agents = xai_grok_agent::discovery::all_subagents_with_plugins(
        cwd,
        &HashMap::new(),
        Some(plugin_registry),
    );

    agents
        .into_iter()
        .map(|a| AgentEntry {
            name: a.name,
            description: a.description,
            source: a.config_source,
        })
        .collect()
}

/// Maps pre-discovered plugins (from `discover_plugins`) to inspect entries.
fn list_plugins(discovered: &[xai_grok_agent::plugins::DiscoveredPlugin]) -> Vec<PluginEntry> {
    discovered
        .iter()
        .map(|p| {
            let scope = match p.scope {
                xai_grok_agent::plugins::PluginScope::CliOverride => Scope::Cli,
                xai_grok_agent::plugins::PluginScope::Project => Scope::Project,
                xai_grok_agent::plugins::PluginScope::User => Scope::User,
                xai_grok_agent::plugins::PluginScope::ConfigPath => Scope::Config,
            };
            PluginEntry {
                name: p.manifest.name.clone(),
                scope,
                path: p.root.display().to_string(),
                enabled: p.trusted,
                provides: PluginProvides {
                    // Count actual SKILL.md files discovered (root-level or in
                    // subdirs), not the number of configured skill dirs, so the
                    // reported count matches what the skills registry loads.
                    skills: xai_grok_agent::plugins::registry::skill_md_paths(&p.skill_dirs).len(),
                    agents: p.agent_dirs.len(),
                    hooks: p.hooks_path.is_some(),
                    mcp_servers: if p.mcp_config_path.is_some() { 1 } else { 0 },
                },
            }
        })
        .collect()
}

/// Wraps the production marketplace resolver (`marketplace::resolve`).
fn list_marketplaces(git_root: Option<&Path>) -> Vec<MarketplaceEntry> {
    let Some(root) = git_root else {
        return vec![];
    };
    xai_grok_agent::plugins::marketplace::resolve(root)
        .into_iter()
        .map(|m| MarketplaceEntry {
            name: m.name,
            path: m.path.display().to_string(),
            enabled_plugins: m.plugin_dirs.len(),
        })
        .collect()
}

/// Discovers MCPs with every vendor enabled so compatibility can be annotated later.
fn list_mcp_servers(
    cwd: &Path,
    plugin_registry: &xai_grok_agent::plugins::PluginRegistry,
) -> Vec<McpServerEntry> {
    use xai_grok_workspace::permission::resolution;

    let all_on = xai_grok_tools::types::compat::CompatConfig::default();
    let sourced = crate::session::managed_mcp::merge_managed_mcp_servers_sourced(
        cwd,
        Some(plugin_registry),
        &all_on,
    );
    let allowlist = &resolution::managed_settings().mcp_allowlist;

    sourced
        .into_iter()
        .map(|(server, source)| {
            let (name, transport, target) =
                match &server {
                    agent_client_protocol::McpServer::Stdio(
                        agent_client_protocol::McpServerStdio { name, command, .. },
                    ) => (
                        name.clone(),
                        "stdio",
                        command
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    ),
                    agent_client_protocol::McpServer::Http(
                        agent_client_protocol::McpServerHttp { name, .. },
                    ) => (name.clone(), "http", String::new()),
                    agent_client_protocol::McpServer::Sse(
                        agent_client_protocol::McpServerSse { name, .. },
                    ) => (name.clone(), "sse", String::new()),
                    // TODO(acp-0.10): `McpServer` is #[non_exhaustive].
                    _ => ("unknown".to_string(), "unknown", String::new()),
                };
            let disabled_reason = (!allowlist.is_server_allowed(&server)).then(|| {
                crate::session::managed_mcp::McpDisabledReason::for_blocked_server(
                    allowlist, &server,
                )
                .to_string()
            });
            let vendor = match &source {
                ConfigSource::ClaudeJson { .. } => Some("claude".to_owned()),
                ConfigSource::McpJson { path } => {
                    derive_vendor(&path.display().to_string()).map(String::from)
                }
                _ => None,
            };
            McpServerEntry {
                name,
                transport: transport.to_string(),
                target,
                source,
                disabled: false,
                compatibility_status: None,
                disabled_reason,
                vendor,
            }
        })
        .collect()
}

/// Wraps the production LSP loader (`load_servers_with_plugins_sourced`).
fn list_lsp_servers(
    cwd: &Path,
    discovered_plugins: &[xai_grok_agent::plugins::DiscoveredPlugin],
) -> Vec<LspServerEntry> {
    let trusted: Vec<_> = discovered_plugins.iter().filter(|p| p.trusted).collect();
    let plugin_lsp_paths: Vec<std::path::PathBuf> = trusted
        .iter()
        .filter_map(|p| p.lsp_config_path.clone())
        .collect();
    let plugin_names: Vec<&str> = trusted
        .iter()
        .filter(|p| p.lsp_config_path.is_some())
        .map(|p| p.manifest.name.as_str())
        .collect();
    let plugin_inline_lsp: Vec<(&serde_json::Value, &str)> = trusted
        .iter()
        .filter_map(|p| {
            p.manifest
                .inline_lsp_servers()
                .map(|v| (v, p.manifest.name.as_str()))
        })
        .collect();
    let inline_values: Vec<&serde_json::Value> =
        plugin_inline_lsp.iter().map(|(v, _)| *v).collect();
    let inline_names: Vec<&str> = plugin_inline_lsp.iter().map(|(_, n)| *n).collect();

    let servers = xai_grok_tools::implementations::lsp::config::load_servers_with_plugins_sourced(
        cwd,
        &plugin_lsp_paths,
        &inline_values,
        &plugin_names,
        &inline_names,
    );

    // Folder-trust gate (display-only): inspect never spawns servers, but mark the
    // repo-local (project-scoped) entries a session would skip in an untrusted
    // clone so the listing matches the live gate. `remote = None` mirrors
    // `grok mcp doctor` (no loaded RemoteSettings in a standalone command).
    crate::agent::folder_trust::resolve_and_record(cwd, None, false);
    let project_allowed = crate::agent::folder_trust::project_scope_allowed(cwd);

    servers
        .into_iter()
        .map(|(name, (cfg, source))| {
            let untrusted = !project_allowed && matches!(source, ConfigSource::Project { .. });
            LspServerEntry {
                name,
                command: cfg.command,
                args: cfg.args,
                source,
                extensions: cfg.extensions.keys().cloned().collect(),
                untrusted,
            }
        })
        .collect()
}

/// Locates the config files that contribute to the effective config by
/// probing the canonical locations used by `ConfigLayers::load` and
/// `requirements_layers`: system + user `managed_config.toml`, user
/// `config.toml`, user + system `requirements.toml`, and project
/// `.grok/config.toml` files (via `find_project_configs`). The macOS MDM
/// managed-preferences layer has no file on disk, so it is sourced directly
/// from `requirements_layers()` rather than a path probe.
///
/// Only on-disk files (plus the synthetic MDM layer) are emitted, except the
/// primary user `config.toml` which always gets a "User: (none)" line in the
/// human view when absent.
/// `note` distinguishes files that exist but contribute nothing after the
/// real loader's processing (stripping, version overrides, fail_closed, etc).
/// Parse errors are reported distinctly rather than as "empty".
fn list_config_sources(cwd: &Path) -> ConfigSources {
    let mut layers: Vec<ConfigLayer> = vec![];

    // System managed (comes first in merge precedence)
    if let Some(dir) = crate::config::system_config_dir() {
        let p = dir.join("managed_config.toml");
        if let Some((path_s, note)) = describe_config_file(&p) {
            layers.push(ConfigLayer {
                role: "system-managed".to_string(),
                path: path_s,
                note,
            });
        }
    }

    // User managed
    if let Some(home) = crate::config::user_grok_home() {
        let p = home.join("managed_config.toml");
        if let Some((path_s, note)) = describe_config_file(&p) {
            layers.push(ConfigLayer {
                role: "managed".to_string(),
                path: path_s,
                note,
            });
        }
    }

    // User config.toml (primary user layer; shown as (none) when absent)
    if let Some(home) = crate::config::user_grok_home() {
        let p = home.join("config.toml");
        if let Some((path_s, note)) = describe_config_file(&p) {
            layers.push(ConfigLayer {
                role: "user".to_string(),
                path: path_s,
                note,
            });
        }
    }

    // Requirements: user then system (order they appear in requirements_layers)
    if let Some(home) = crate::config::user_grok_home() {
        let p = home.join("requirements.toml");
        if let Some((path_s, note)) = describe_requirements_file(&p) {
            layers.push(ConfigLayer {
                role: "requirements".to_string(),
                path: path_s,
                note,
            });
        }
    }
    if let Some(dir) = crate::config::system_config_dir() {
        let p = dir.join("requirements.toml");
        if let Some((path_s, note)) = describe_requirements_file(&p) {
            layers.push(ConfigLayer {
                role: "system-requirements".to_string(),
                path: path_s,
                note,
            });
        }
    }

    // macOS MDM managed preferences: a synthetic, admin-forced requirements layer
    // with no file on disk, so it's sourced from requirements_layers() (keyed on
    // the synthetic label) with contribution decided from the in-memory value
    // rather than a path probe. Absent on non-macOS or when no profile is forced.
    let rt_layers = crate::config::requirements_layers();
    if let Some(mdm) = rt_layers
        .iter()
        .find(|l| matches!(l.source, crate::config::RequirementsSource::Mdm))
    {
        let path_s = mdm.source.label().into_owned();
        let note = if requirements_layer_contributes(&rt_layers, &path_s) {
            None
        } else {
            Some("empty".to_string())
        };
        layers.push(ConfigLayer {
            role: "mdm".to_string(),
            path: path_s,
            note,
        });
    }

    // Project configs (from git root up); each is its own "project" role entry
    for p in crate::config::find_project_configs(cwd) {
        if p.exists()
            && let Some((path_s, note)) = describe_config_file(&p)
        {
            layers.push(ConfigLayer {
                role: "project".to_string(),
                path: path_s,
                note,
            });
        }
    }

    ConfigSources { layers }
}

/// For managed / user / project config files: use `load_config_file` (the
/// production path for those layers) so `note` reflects post-processing
/// (version overrides stripped) and distinguishes parse failure.
fn describe_config_file(path: &Path) -> Option<(String, Option<String>)> {
    if !path.exists() {
        return None;
    }
    let path_s = path.display().to_string();
    match crate::config::load_config_file(path) {
        Ok(v) => {
            let empty = v.as_table().is_none_or(|t| t.is_empty());
            Some((
                path_s,
                if empty {
                    Some("empty".to_string())
                } else {
                    None
                },
            ))
        }
        Err(_) => Some((path_s, Some("parse error".to_string()))),
    }
}

/// Classify a requirements file against the real loader. `load_config_file`
/// catches both syntax errors and invalid `[[version_overrides]]` (the loader
/// rejects the latter too), so those read "(parse error)"; contribution is
/// then sourced from `requirements_layers()` via `requirements_layer_contributes`.
fn describe_requirements_file(path: &Path) -> Option<(String, Option<String>)> {
    if !path.exists() {
        return None;
    }
    let path_s = path.display().to_string();
    if crate::config::load_config_file(path).is_err() {
        return Some((path_s, Some("parse error".to_string())));
    }
    if requirements_layer_contributes(&crate::config::requirements_layers(), &path_s) {
        Some((path_s, None))
    } else {
        Some((path_s, Some("empty".to_string())))
    }
}

/// Whether the loader keeps `path_s` *and* its post-load table is non-empty.
/// The non-empty guard runs before `fail_closed` is stripped, so a
/// `fail_closed`-only file is retained with an empty table yet contributes nothing.
fn requirements_layer_contributes(
    layers: &[crate::config::RequirementsLayer],
    path_s: &str,
) -> bool {
    layers.iter().any(|l| {
        l.source.label().as_ref() == path_s && l.value.as_table().is_some_and(|t| !t.is_empty())
    })
}

fn print_section<T>(title: &str, items: &[T], format_item: impl Fn(&T) -> String) {
    println!();
    println!("  {} ({})", title, items.len());
    if items.is_empty() {
        println!("  {TREE} (none)");
    }
    for item in items {
        println!("  {TREE} {}", format_item(item));
    }
}

/// Print items in a two-column layout: name on the left, source label on the right.
fn print_columns<T>(
    title: &str,
    items: &[T],
    name: impl Fn(&T) -> String,
    label: impl Fn(&T) -> String,
) {
    println!();
    println!("  {} ({})", title, items.len());
    if items.is_empty() {
        println!("  {TREE} (none)");
        return;
    }
    let names: Vec<String> = items.iter().map(&name).collect();
    let pad = names.iter().map(|n| n.len()).max().unwrap_or(0).min(50);
    for (item, n) in items.iter().zip(&names) {
        println!("  {TREE} {:<pad$}  {}", n, label(item));
    }
}

/// Render the team pin for the human view: single value, comma-joined list,
/// or an explicit empty-list marker (which fails closed at login).
fn format_force_login_team(team: &Option<ForceLoginTeam>) -> String {
    match team {
        None => "(none)".to_string(),
        Some(ForceLoginTeam::Single(s)) => s.clone(),
        Some(ForceLoginTeam::AnyOf(list)) if list.is_empty() => {
            "(empty -- fail closed)".to_string()
        }
        Some(ForceLoginTeam::AnyOf(list)) => list.join(", "),
    }
}

/// Human label for an enforced setting. Uses product vocabulary, not the
/// internal field names (no `ui.yolo` / `--yolo` / `permission_mode`).
fn enforced_label(p: &EnforcedPolicy) -> String {
    let name = match p.setting.as_str() {
        "alwaysApprove" => "Permissions mode: always-approve",
        "telemetry" => "Telemetry",
        "feedback" => "Feedback",
        other => other,
    };
    let state = if p.enabled { "enabled" } else { "disabled" };
    format!("{name} {state}")
}

fn disabled_compat_tags(
    disabled: bool,
    compatibility_status: Option<CompatEntryStatus>,
) -> &'static str {
    if disabled || compatibility_status == Some(CompatEntryStatus::Disabled) {
        " [disabled]"
    } else {
        ""
    }
}

/// Human role label for a config layer. Source classification stays actionable
/// without emitting the on-disk path.
fn config_layer_role_label(role: &str) -> &str {
    match role {
        "system-managed" => "System Managed",
        "managed" => "Managed",
        "system-requirements" => "System Requirements",
        "requirements" => "Requirements",
        "mdm" => "MDM Requirements",
        "project" => "Project",
        "user" => "User",
        other => other,
    }
}

fn config_layer_note_tag(note: Option<&str>) -> &'static str {
    match note {
        Some("empty") => " (empty)",
        Some("parse error") => " (parse error)",
        _ => "",
    }
}

/// Role + basename (plus empty/parse-error tag). Never an absolute path.
fn config_layer_human_line(layer: &ConfigLayer) -> String {
    format!(
        "{}: {}{}",
        config_layer_role_label(&layer.role),
        inspect_path_label(&layer.path),
        config_layer_note_tag(layer.note.as_deref()),
    )
}

fn render_config_warnings(
    warnings: &[crate::agent::config_model_override_parse::ConfigWarning],
) -> String {
    use std::fmt::Write as _;

    if warnings.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n  Config Warnings\n");
    let _ = writeln!(out, "  {TREE} {} warning(s)", warnings.len());
    for w in warnings {
        let field = w.field().map(|f| format!(" {f}")).unwrap_or_default();
        let _ = writeln!(
            out,
            "    {TREE} [{}]{field} — {}",
            w.target.label(),
            w.reason
        );
    }
    out
}

fn render_harness_compatibility(report: &ExternalCompatReport) -> String {
    use std::fmt::Write as _;

    let mut out = String::from("\n  Harness Compatibility\n");
    let mut current_vendor = "";
    for cell in &report.cells {
        if cell.vendor != current_vendor {
            current_vendor = &cell.vendor;
            let _ = writeln!(out, "  {TREE} {current_vendor}");
        }
        let status = if cell.enabled { "on" } else { "OFF" };
        let _ = writeln!(
            out,
            "    {TREE} {:<10} {:<3}  ({})",
            cell.surface, status, cell.source
        );
    }
    out.push('\n');
    out
}

fn print_human(r: &InspectReport) {
    println!();
    println!("  Environment");
    println!("  {TREE} Version: {} [{}]", r.grok_version, r.channel);
    println!("  {TREE} CWD: {}", r.cwd);
    if let Some(ref root) = r.project_root {
        println!("  {TREE} Git root: {}", root);
    }
    println!(
        "  {TREE} Project trusted: {}",
        if r.project_trusted { "yes" } else { "no" }
    );

    print_section("Project Instructions", &r.project_instructions, |f| {
        let status = disabled_compat_tags(f.disabled, f.compatibility_status);
        format!(
            "{} ({}, ~{} tokens){}{}",
            inspect_path_label(&f.path),
            f.scope,
            f.approx_tokens,
            vendor_tag(&f.vendor),
            status,
        )
    });

    println!();
    println!("  Permissions");
    if r.permissions.managed_settings_exists
        && let Some(ref p) = r.permissions.managed_settings_path
    {
        let status = if r.permissions.managed_settings_active {
            "active"
        } else {
            "not loaded"
        };
        println!(
            "  {TREE} Managed settings: {} ({status})",
            inspect_path_label(p)
        );
    }
    if r.permissions.sources.is_empty() {
        println!("  {TREE} Source: (none)");
    } else {
        for src in &r.permissions.sources {
            println!("  {TREE} Source: {src}");
        }
    }
    println!(
        "  {TREE} {} loaded, {} skipped",
        r.permissions.loaded,
        r.permissions.skipped.len()
    );
    for s in &r.permissions.skipped {
        println!("    {TREE} {} -- {}", s.rule, s.reason);
    }
    if !r.permissions.enforced.is_empty() {
        println!("  {TREE} Enforced by policy");
        for e in &r.permissions.enforced {
            println!("    {TREE} {} ({})", enforced_label(e), e.source);
        }
    }
    if !r.permissions.mcp_server_allowlist.is_empty() {
        println!(
            "  {TREE} MCP server allowlist ({} patterns)",
            r.permissions.mcp_server_allowlist.len()
        );
        for pat in &r.permissions.mcp_server_allowlist {
            println!("    {TREE} {}", pat);
        }
    }
    if !r.permissions.marketplace_allowlist.is_empty() {
        println!(
            "  {TREE} Marketplace allowlist ({} sources)",
            r.permissions.marketplace_allowlist.len()
        );
        for url in &r.permissions.marketplace_allowlist {
            println!("    {TREE} {}", url);
        }
    }

    println!();
    println!("  Login Policy");
    println!(
        "  {TREE} disable_api_key_auth: {}",
        match r.login_policy.disable_api_key_auth {
            Some(v) => v.to_string(),
            None => "(unset)".to_string(),
        }
    );
    println!(
        "  {TREE} force_login_team_uuid: {}",
        format_force_login_team(&r.login_policy.force_login_team_uuid)
    );
    println!(
        "  {TREE} api_key_auth_disabled: {}",
        r.login_policy.api_key_auth_disabled
    );

    print_columns(
        "Skills",
        &r.skills,
        |s| s.name.clone(),
        |s| {
            let status = disabled_compat_tags(s.disabled, s.compatibility_status);
            if s.quarantined {
                let codes = s.diagnostic_codes.join(",");
                format!(
                    "{}{}{} quarantined [{}]",
                    s.source.display_label(),
                    vendor_tag(&s.vendor),
                    status,
                    codes
                )
            } else {
                format!(
                    "{}{}{}",
                    s.source.display_label(),
                    vendor_tag(&s.vendor),
                    status,
                )
            }
        },
    );

    print_columns(
        "Agents",
        &r.agents,
        |a| a.name.clone(),
        |a| a.source.display_label(),
    );

    print_columns(
        "Plugins",
        &r.plugins,
        |p| {
            let status = if p.enabled { "enabled" } else { "disabled" };
            format!("{} ({}, {})", p.name, p.scope, status)
        },
        |p| {
            let mut parts = Vec::new();
            if p.provides.skills > 0 {
                parts.push(format!("{} skills", p.provides.skills));
            }
            if p.provides.agents > 0 {
                parts.push(format!("{} agents", p.provides.agents));
            }
            if p.provides.hooks {
                parts.push("hooks".into());
            }
            if p.provides.mcp_servers > 0 {
                parts.push(format!("{} MCPs", p.provides.mcp_servers));
            }
            if parts.is_empty() {
                "-".into()
            } else {
                parts.join(", ")
            }
        },
    );

    print_section("Marketplaces", &r.marketplaces, |m| {
        format!(
            "{} ({}, {} enabled plugins)",
            m.name,
            inspect_path_label(&m.path),
            m.enabled_plugins
        )
    });

    if r.mcp_servers.is_empty() {
        println!();
        println!("  MCP Servers (0)");
        println!("  {TREE} (none) \u{2014} see `grok mcp add --help`");
    } else {
        print_columns(
            "MCP Servers",
            &r.mcp_servers,
            |m| {
                if let Some(ref reason) = m.disabled_reason {
                    format!("{} ({}) [BLOCKED: {}]", m.name, m.transport, reason)
                } else {
                    format!("{} ({})", m.name, m.transport)
                }
            },
            |m| {
                let status = disabled_compat_tags(m.disabled, m.compatibility_status);
                format!(
                    "{}{}{}",
                    m.source.display_label(),
                    vendor_tag(&m.vendor),
                    status,
                )
            },
        );
    }

    print_columns(
        "LSP Servers",
        &r.lsp_servers,
        |l| {
            format!(
                "{} ({} {})",
                l.name,
                inspect_path_label(&l.command),
                l.args.join(" ")
            )
        },
        |l| {
            let untrusted = if l.untrusted { " [untrusted]" } else { "" };
            format!("{}{}", l.source.display_label(), untrusted)
        },
    );

    print_columns(
        "Hooks",
        &r.hooks,
        |h| {
            let matcher = h
                .matcher
                .as_ref()
                .map(|m| format!(" matcher={}", m))
                .unwrap_or_default();
            format!("{}{}", h.hook_type, matcher)
        },
        |h| {
            let status = disabled_compat_tags(h.disabled, h.compatibility_status);
            format!(
                "{}{}{}",
                h.source.display_label(),
                vendor_tag(&h.vendor),
                status,
            )
        },
    );

    println!();
    println!("  Config Sources");
    // User is always emitted (with (none) when absent) for the primary user config.
    if let Some(user_l) = r.config_sources.layers.iter().find(|l| l.role == "user") {
        println!("  {TREE} {}", config_layer_human_line(user_l));
    } else {
        println!("  {TREE} User: (none)");
    }
    for layer in &r.config_sources.layers {
        if layer.role == "user" {
            continue;
        }
        println!("  {TREE} {}", config_layer_human_line(layer));
    }
    if !r.config_sources.layers.iter().any(|l| l.role == "project") {
        println!("  {TREE} Project: (none)");
    }

    print!("{}", render_config_warnings(&r.config_warnings));

    print!("{}", render_harness_compatibility(&r.external_compat));

    if let Some(p) = &r.provider_registry {
        print_provider_registry(p);
    }
    if let Some(m) = &r.model_catalog {
        print_model_catalog(m);
    }
    if let Some(rt) = &r.retrieval {
        print_retrieval(rt);
    }
}

fn credential_status_label(s: CredentialStatus) -> &'static str {
    use CredentialStatus as S;
    match s {
        S::Configured => "configured",
        S::Environment => "environment",
        S::Oauth => "oauth",
        S::Helper => "helper",
        S::Missing => "missing",
        S::Unavailable => "unavailable",
    }
}

fn print_provider_registry(p: &ProviderRegistryInspect) {
    println!();
    println!("  Providers ({})", p.providers.len());
    if let Some(ref diag) = p.diagnostic {
        println!("  {TREE} {diag}");
        return;
    }
    println!("  {TREE} Registry generation: {}", p.generation);
    for row in &p.providers {
        let inc = row
            .incarnation
            .as_deref()
            .map(|i| format!(", incarnation {}", i.chars().take(8).collect::<String>()))
            .unwrap_or_default();
        let state = if row.enabled { "enabled" } else { "disabled" };
        let builtin = if row.is_built_in {
            "built-in"
        } else {
            "configured"
        };
        let tomb = if row.tombstoned == Some(true) {
            " [tombstoned]"
        } else {
            ""
        };
        println!(
            "  {TREE} {} ({kind}, {builtin}, {state}{tomb}){inc}",
            row.id,
            kind = row.kind,
        );
        println!(
            "    {TREE} surfaces: {} | credential: {}",
            row.api_surfaces.join(", "),
            credential_status_label(row.credential_status)
        );
        if !row.credential_routes.is_empty() {
            println!("    {TREE} routes: {}", row.credential_routes.join(", "));
        }
        if let Some(ref c) = row.catalog {
            println!(
                "    {TREE} catalog: {} model(s), validity {}",
                c.model_count
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".into()),
                cache_validity_label(c.validity)
            );
        }
        if let Some(ref cap) = row.capability {
            println!(
                "    {TREE} capability: validity {}",
                cache_validity_label(cap.validity)
            );
        }
        if let Some(ref r) = row.references {
            print!("    {TREE} references: can_remove={}", r.can_remove);
            if !r.groups.is_empty() {
                let pairs: Vec<String> = r
                    .groups
                    .iter()
                    .filter(|g| g.count > 0)
                    .map(|g| format!("{}={}", g.kind, g.count))
                    .collect();
                if !pairs.is_empty() {
                    println!(" ({})", pairs.join(", "));
                } else {
                    println!();
                }
            } else {
                println!();
            }
        }
    }
}

fn cache_validity_label(v: CacheValidity) -> &'static str {
    use CacheValidity as V;
    match v {
        V::Valid => "valid",
        V::Mismatch => "mismatch",
        V::Corrupt => "corrupt",
        V::Tombstoned => "tombstoned",
        V::Unavailable => "unavailable",
        V::NotChecked => "not_checked",
    }
}

fn print_model_catalog(m: &ModelCatalogInspect) {
    println!();
    println!("  Model Catalog ({})", m.total_visible_count);
    println!(
        "  {TREE} auth view: {} (OAuth-only subset excluded; credential presence may be read, values are never emitted)",
        m.auth_view
    );
    if let Some(ref diag) = m.diagnostic {
        println!("  {TREE} {diag}");
        return;
    }
    match m.models_cache.as_str() {
        "valid" => println!("  {TREE} models_cache.json: valid; contributed to this view"),
        "absent" => {
            println!("  {TREE} models_cache.json: absent; showing built-in defaults + config")
        }
        status => println!(
            "  {TREE} models_cache.json: {status}; rejected, showing built-in defaults + config"
        ),
    }
    if let Some(ref d) = m.default {
        println!("  {TREE} default: {} (source: {})", d.id, d.source);
    }
    if !m.duplicate_groups.is_empty() {
        println!(
            "  {TREE} duplicate upstream groups: {}",
            m.duplicate_groups.len()
        );
        for g in m.duplicate_groups.iter().take(MAX_NAMES) {
            println!(
                "    {TREE} {} -> {}",
                g.upstream_id,
                g.canonical_ids.join(", ")
            );
        }
    }
    if !m.aliases.is_empty() {
        println!("  {TREE} aliases: {}", m.aliases.len());
        for a in m.aliases.iter().take(MAX_NAMES) {
            let mut line = format!("    {TREE} {} -> {}", a.input, a.kind);
            if let Some(ref id) = a.canonical_id {
                line.push_str(&format!(" ({id})"));
            } else if !a.candidates.is_empty() {
                line.push_str(&format!(" ({})", a.candidates.join(", ")));
            }
            println!("{line}");
        }
    }
    if !m.gated_entries.is_empty() {
        println!(
            "  {TREE} gated (multi-account rollout off): {}",
            m.gated_entries.join(", ")
        );
    }
    for w in &m.warnings {
        println!("  {TREE} warning: {w}");
    }
}

fn print_retrieval(r: &RetrievalInspect) {
    println!();
    println!("  Retrieval & Prime");
    let validity = match r.validity {
        RetrievalValidity::Valid => "valid",
        RetrievalValidity::Invalid => "invalid",
        RetrievalValidity::Disabled => "disabled",
    };
    println!(
        "  {TREE} validity: {validity} | source: {} | enabled: {}",
        r.source, r.enabled
    );
    if let Some(ref diag) = r.diagnostic {
        println!("  {TREE} {diag}");
    }
    if r.last_known_good_retained == Some(true) {
        println!("  {TREE} disk candidate invalid; published last-known-good retained");
    }
    println!(
        "  {TREE} snapshot gen {}, graph gen {}, provider gen {}",
        r.generation, r.graph_generation, r.provider_generation
    );
    if let Some(ref fp) = r.fingerprint {
        println!("  {TREE} fingerprint: {}", inspect_fingerprint_short(fp));
    }
    if !r.embedding_models.is_empty() {
        println!(
            "  {TREE} embedding models: {}",
            r.embedding_models
                .iter()
                .map(|e| format!("{} ({}/{})", e.id, e.provider_instance_id, e.model))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !r.reranker_models.is_empty() {
        println!(
            "  {TREE} reranker models: {}",
            r.reranker_models
                .iter()
                .map(|e| format!("{} ({}/{})", e.id, e.provider_instance_id, e.model))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    println!(
        "  {TREE} profiles: {}",
        r.profiles
            .iter()
            .map(|p| format!(
                "{} (emb={}, rr={}, deadline {}ms)",
                p.id,
                p.embedding_route_ids.join(","),
                p.reranker_route_ids.join(","),
                p.budgets.deadline_ms
            ))
            .collect::<Vec<_>>()
            .join("; ")
    );
    println!(
        "  {TREE} prime: skills.enabled={} agents.enabled={}{}{}",
        r.prime.skills.enabled,
        r.prime.agents.enabled,
        r.prime
            .skills
            .retrieval_profile
            .as_deref()
            .map(|p| format!(", skills.profile={p}"))
            .unwrap_or_default(),
        r.prime
            .agents
            .retrieval_profile
            .as_deref()
            .map(|p| format!(", agents.profile={p}"))
            .unwrap_or_default(),
    );
    if let Some(p) = &r.memory_retrieval_profile {
        println!("  {TREE} memory retrieval profile: {p}");
    }
    if let Some(pin) = &r.memory_pin {
        let route_state = if pin.configured_route_matches_installed {
            "configured route matches"
        } else {
            "configured route changed; rebuild required"
        };
        println!(
            "  {TREE} memory pin: {} -> {}/{} (space {}, pinned; {})",
            pin.configured_profile,
            pin.installed_provider_instance_id,
            pin.installed_model,
            pin.embedding_space_fingerprint_short,
            route_state
        );
    }
    for w in &r.warnings {
        println!("  {TREE} warning: {w}");
    }
    if let Some(idx) = &r.prime_index {
        println!(
            "  {TREE} prime index gen {} space {} skills {}/{} ({}) agents {}/{} ({}){}",
            idx.generation,
            idx.fingerprint_short,
            idx.skills_vectors,
            idx.skills_items,
            idx.skills_readiness,
            idx.agents_vectors,
            idx.agents_items,
            idx.agents_readiness,
            idx.job_state
                .as_deref()
                .map(|s| format!(" job {s}"))
                .unwrap_or_default(),
        );
        if let Some(route) = &idx.configured_route {
            println!("  {TREE} prime index route {route}");
        }
    }
    for mirror in &r.vector_mirrors {
        println!(
            "  {TREE} vector mirror {} {} [{}] (rows {}) — sqlite-vec fallback on any error",
            mirror.backend,
            mirror.collection,
            mirror.state,
            mirror
                .row_count
                .map(|n| n.to_string())
                .unwrap_or_else(|| "?".into()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_agent::prompt::skills::{SkillInfo, SkillsConfig};
    use xai_grok_tools::implementations::skills::types::SkillScope;

    #[test]
    fn harness_compatibility_human_output_stays_compact() {
        let effective_config: toml::Value =
            toml::from_str("[compat.cursor]\nrules = false").unwrap();
        let report = compat::resolve_inspect_compat_with_env(Ok(&effective_config), |_| None);

        let human = render_harness_compatibility(&report);

        assert!(human.contains("skills     on   (default)"), "{human}");
        assert!(human.contains("rules      OFF  (config)"), "{human}");
        assert!(
            !human.contains("Defaults shown; remote may override."),
            "{human}"
        );
        assert!(!human.contains("resolved at session start"), "{human}");
        assert!(!human.contains("unresolved"), "{human}");
        assert!(!human.contains("?"), "{human}");
    }

    #[test]
    fn disabled_entry_status_serializes_and_renders_consistently() {
        let entry = InstructionFile {
            path: "/repo/.cursor/AGENTS.md".to_owned(),
            scope: Scope::Project,
            file_type: "agents_md".to_owned(),
            size_bytes: 10,
            approx_tokens: 3,
            vendor: Some("cursor".to_owned()),
            disabled: false,
            compatibility_status: Some(CompatEntryStatus::Disabled),
        };
        assert_eq!(
            serde_json::to_value(&entry).unwrap(),
            serde_json::json!({
                "path": "AGENTS.md",
                "scope": "project",
                "fileType": "agents_md",
                "sizeBytes": 10,
                "approxTokens": 3,
                "vendor": "cursor",
                "compatibilityStatus": "disabled"
            })
        );
        assert_eq!(
            disabled_compat_tags(false, entry.compatibility_status),
            " [disabled]"
        );
        assert_eq!(
            format!(
                "{} ({}, ~{} tokens){}{}",
                inspect_path_label(&entry.path),
                entry.scope,
                entry.approx_tokens,
                vendor_tag(&entry.vendor),
                disabled_compat_tags(false, entry.compatibility_status),
            ),
            "AGENTS.md (project, ~3 tokens) [cursor] [disabled]"
        );
    }

    #[test]
    fn inspect_path_label_is_basename_only() {
        assert_eq!(inspect_path_label("/repo/.cursor/AGENTS.md"), "AGENTS.md");
        assert_eq!(
            inspect_path_label(r"C:\repo\.cursor\AGENTS.md"),
            "AGENTS.md"
        );
        assert_eq!(inspect_path_label("AGENTS.md"), "AGENTS.md");
        assert_eq!(
            inspect_path_label("/Users/me/.grok/config.toml"),
            "config.toml"
        );
    }

    #[test]
    fn config_sources_human_uses_role_and_basename() {
        let user = ConfigLayer {
            role: "user".into(),
            path: "/Users/me/.grok/config.toml".into(),
            note: Some("empty".into()),
        };
        let project = ConfigLayer {
            role: "project".into(),
            path: "/repo/.grok/config.toml".into(),
            note: None,
        };
        let managed = ConfigLayer {
            role: "managed".into(),
            path: r"C:\Users\me\.grok\managed_config.toml".into(),
            note: Some("parse error".into()),
        };
        assert_eq!(config_layer_human_line(&user), "User: config.toml (empty)");
        assert_eq!(config_layer_human_line(&project), "Project: config.toml");
        assert_eq!(
            config_layer_human_line(&managed),
            "Managed: managed_config.toml (parse error)"
        );
        for line in [
            config_layer_human_line(&user),
            config_layer_human_line(&project),
            config_layer_human_line(&managed),
        ] {
            assert!(!line.contains("/Users/"), "{line}");
            assert!(!line.contains("/repo/"), "{line}");
            assert!(!line.contains(r"C:\"), "{line}");
            assert!(!line.contains(".grok/"), "{line}");
            assert!(!line.contains(r".grok\"), "{line}");
        }
    }

    #[test]
    fn vendor_rule_paths_select_rules_compatibility_cells() {
        let cell = |vendor: &str, surface: &str, enabled: bool| ExternalCompatEntry {
            vendor: vendor.to_owned(),
            surface: surface.to_owned(),
            enabled,
            source: CompatSource::Config,
        };
        let report = ExternalCompatReport {
            remote_settings_loaded: false,
            cells: vec![
                cell("cursor", "rules", false),
                cell("cursor", "agents", true),
                cell("claude", "rules", false),
                cell("claude", "agents", true),
            ],
        };

        for (vendor, path) in [
            ("cursor", "/repo/.cursor/rules/team.md"),
            ("cursor", r"C:\repo\.cursor\rules\team.md"),
            ("claude", "/repo/.claude/rules/team.md"),
            ("claude", r"C:\repo\.claude\rules\team.md"),
        ] {
            let file_type = instruction_file_type(path, Path::new("/home/user/.grok"), false, &[]);
            assert_eq!(file_type, "rules");
            assert_eq!(
                instruction_compat_status(&Some(vendor.to_owned()), file_type, &report),
                Some(CompatEntryStatus::Disabled)
            );
        }

        for path in ["/repo/.grok/rules/team.md", r"C:\repo\.grok\rules\team.md"] {
            assert_eq!(
                instruction_file_type(path, Path::new("/home/user/.grok"), false, &[]),
                "rules"
            );
        }
        for path in [
            "/repo/.cursor/rules/team.md",
            r"C:\repo\.cursor\rules\team.md",
        ] {
            assert_eq!(
                instruction_file_type(path, Path::new("/home/user/.grok"), true, &[]),
                "rules"
            );
        }
        for path in [
            "/repo/.claude/rules/team.md",
            r"C:\repo\.claude\rules\team.md",
        ] {
            let file_type = instruction_file_type(path, Path::new("/home/user/.grok"), true, &[]);
            assert_eq!(file_type, "agents_md");
            assert_eq!(
                instruction_compat_status(&Some("claude".to_owned()), file_type, &report),
                Some(CompatEntryStatus::Enabled)
            );
        }
        for path in [
            "/repo/not.cursor/rules/team.md",
            r"C:\repo\.cursor\ruleset\team.md",
        ] {
            assert_eq!(
                instruction_file_type(path, Path::new("/home/user/.grok"), false, &[]),
                "agents_md"
            );
        }
    }

    #[test]
    fn grok_home_nested_in_workspace_keeps_direct_surfaces_global() {
        let grok_home = Path::new("/repo/config");
        let workspace = Path::new("/repo");
        for path in ["/repo/config/AGENTS.md", "/repo/config/rules/global.md"] {
            assert!(matches!(
                instruction_scope(path, grok_home, &[], workspace),
                Scope::Global
            ));
        }
        for path in [
            "/repo/config/.grok/rules/project.md",
            "/repo/config/src/AGENTS.md",
        ] {
            assert!(matches!(
                instruction_scope(path, grok_home, &[], workspace),
                Scope::Project
            ));
        }
    }

    #[test]
    fn vendor_home_nested_in_workspace_keeps_direct_surfaces_global() {
        let vendor_homes = vec![(Path::new("/repo/.claude").to_path_buf(), true)];
        let workspace = Path::new("/repo");
        for path in ["/repo/.claude/rules/global.md", "/repo/.claude/CLAUDE.md"] {
            assert!(matches!(
                instruction_scope(path, Path::new("/other/grok"), &vendor_homes, workspace),
                Scope::Global
            ));
        }
        for path in [
            "/repo/.claude/.claude/rules/project.md",
            "/repo/.claude/src/AGENTS.md",
        ] {
            assert!(matches!(
                instruction_scope(path, Path::new("/other/grok"), &vendor_homes, workspace),
                Scope::Project
            ));
        }
    }

    #[test]
    fn workspace_scope_wins_inside_grok_home() {
        let grok_home = Path::new("/custom/grok");
        let workspace = Path::new("/custom/grok/worktrees/repo");
        for path in [
            "/custom/grok/worktrees/repo/.cursor/rules/project.md",
            "/custom/grok/worktrees/repo/src/AGENTS.md",
        ] {
            assert!(matches!(
                instruction_scope(path, grok_home, &[], workspace),
                Scope::Project
            ));
        }
        assert!(matches!(
            instruction_scope("/custom/grok/rules/global.md", grok_home, &[], workspace,),
            Scope::Global
        ));
    }

    #[test]
    fn custom_grok_home_rules_are_classified_as_rules() {
        assert_eq!(
            instruction_file_type(
                "/custom/config/rules/team.md",
                Path::new("/custom/config"),
                false,
                &[],
            ),
            "rules"
        );
        assert_eq!(
            instruction_file_type(
                "/custom/config/AGENTS.md",
                Path::new("/custom/config"),
                false,
                &[],
            ),
            "agents_md"
        );
    }

    #[test]
    fn describe_config_file_flags_empty_and_parse_error() {
        let dir = tempfile::tempdir().unwrap();

        // Missing file: describe returns None (no layer entry).
        let missing = dir.path().join("missing.toml");
        assert!(describe_config_file(&missing).is_none());

        // Comment-only and whitespace-only files parse to an empty table after load.
        let comment_only = dir.path().join("comment.toml");
        std::fs::write(&comment_only, "# nothing enforced here\n").unwrap();
        let (_, note) = describe_config_file(&comment_only).unwrap();
        assert_eq!(note.as_deref(), Some("empty"));

        let blank = dir.path().join("blank.toml");
        std::fs::write(&blank, "\n\n").unwrap();
        let (_, note) = describe_config_file(&blank).unwrap();
        assert_eq!(note.as_deref(), Some("empty"));

        // A file with real content contributes config and has no note.
        let with_content = dir.path().join("content.toml");
        std::fs::write(&with_content, "[telemetry]\nmode = \"disabled\"\n").unwrap();
        let (_, note) = describe_config_file(&with_content).unwrap();
        assert!(note.is_none());

        // Malformed TOML is flagged as parse error (distinct from empty).
        let bad = dir.path().join("bad.toml");
        std::fs::write(&bad, "[[[ this is not valid toml").unwrap();
        let (_, note) = describe_config_file(&bad).unwrap();
        assert_eq!(note.as_deref(), Some("parse error"));
    }

    #[test]
    fn describe_requirements_file_flags_invalid_version_overrides_as_parse_error() {
        // Valid TOML but invalid `[[version_overrides]]` is rejected by the real
        // loader, so it must read "parse error", not "empty".
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("requirements.toml");
        std::fs::write(&path, "[[version_overrides]]\nminimum_version = \"nope\"\n").unwrap();
        let (_, note) = describe_requirements_file(&path).unwrap();
        assert_eq!(note.as_deref(), Some("parse error"));
    }

    #[test]
    fn requirements_layer_contributes_requires_non_empty_post_strip_table() {
        // A `fail_closed`-only file is kept by the loader but with an empty
        // post-strip table, so it must not count as contributing.
        let path = "/home/u/.grok/requirements.toml";
        let layer = |v| crate::config::RequirementsLayer {
            value: v,
            source: crate::config::RequirementsSource::File(std::path::PathBuf::from(path)),
            is_system: false,
        };
        let empty = layer(toml::Value::Table(toml::map::Map::new()));
        assert!(!requirements_layer_contributes(
            std::slice::from_ref(&empty),
            path
        ));

        let mut tbl = toml::map::Map::new();
        tbl.insert("telemetry".into(), toml::Value::Boolean(true));
        let full = layer(toml::Value::Table(tbl));
        assert!(requirements_layer_contributes(
            std::slice::from_ref(&full),
            path
        ));
    }

    #[test]
    fn enforced_label_uses_product_vocabulary() {
        let p = EnforcedPolicy {
            setting: "alwaysApprove".into(),
            enabled: false,
            source: "managed-settings.json".into(),
        };
        assert_eq!(
            enforced_label(&p),
            "Permissions mode: always-approve disabled"
        );
        assert!(!enforced_label(&p).contains("yolo"));
    }

    /// Model-override warnings flow from an effective config through `Config`
    /// to the human renderer and the JSON report.
    #[test]
    fn config_warnings_inspect_smoke() {
        let effective: toml::Value = toml::from_str(
            r#"
            [model."grok-4.5"]
            model = "grok-4.5"
            env_key = "ANTHROPIC_AUTH_TOKEN"
            compactions_remaining = 1
            send_compactions_remaining = true
            reasoning_effort = "not-a-level"
            "#,
        )
        .unwrap();
        let cfg = crate::agent::config::Config::new_from_toml_cfg(&effective).unwrap();
        let warnings = cfg.config_warnings;
        assert!(
            warnings
                .iter()
                .any(|w| w.field() == Some("send_compactions_remaining")),
            "duplicate alias should warn: {warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.field() == Some("reasoning_effort")),
            "invalid enum should warn: {warnings:?}"
        );
        assert!(cfg.config_models.contains_key("grok-4.5"));

        let human = render_config_warnings(&warnings);
        assert!(human.contains("Config Warnings"), "{human}");
        assert!(
            human.contains("[model.\"grok-4.5\"] send_compactions_remaining"),
            "{human}"
        );
        assert!(
            human.contains("[model.\"grok-4.5\"] reasoning_effort"),
            "{human}"
        );
        // Auth-provider warnings render under their own table syntax.
        let provider_warning =
            crate::agent::config_model_override_parse::ConfigWarning::auth_provider(
                "litellm",
                Some("command"),
                crate::agent::config_model_override_parse::ConfigWarningKind::InvalidValue,
                "missing or empty command".to_owned(),
            );
        let human = render_config_warnings(&[provider_warning]);
        assert!(
            human.contains("[auth_provider.\"litellm\"] command"),
            "{human}"
        );
        // A dotted provider name renders whole; the field splits off the
        // right.
        let dotted = crate::agent::config_model_override_parse::ConfigWarning::auth_provider(
            "corp.gateway",
            Some("token_ttl_secs"),
            crate::agent::config_model_override_parse::ConfigWarningKind::InvalidValue,
            "at or below the refresh margin".to_owned(),
        );
        let human = render_config_warnings(&[dotted]);
        assert!(
            human.contains("[auth_provider.\"corp.gateway\"] token_ttl_secs"),
            "{human}"
        );
        assert_eq!(render_config_warnings(&[]), "");

        let json = serde_json::to_value(&warnings).unwrap();
        let alias_warning = json
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["field"] == "send_compactions_remaining")
            .expect("alias warning present in JSON");
        assert_eq!(alias_warning["target"], "model");
        assert_eq!(alias_warning["key"], "grok-4.5");
        assert_eq!(alias_warning["kind"], "duplicate-alias");
        assert!(
            alias_warning["reason"]
                .as_str()
                .is_some_and(|r| !r.is_empty())
        );
    }

    // ── skill source mapping (skill_entry_source) ─────────────────────────

    fn skill_fixture(name: &str, path: &str, scope: SkillScope) -> SkillInfo {
        SkillInfo {
            name: name.to_string(),
            description: format!("desc for {name}"),
            path: path.to_string(),
            scope,
            ..SkillInfo::default()
        }
    }

    #[test]
    fn skill_entry_source_maps_scopes() {
        let s = skill_fixture("a", "/repo/.grok/skills/a/SKILL.md", SkillScope::Local);
        assert!(matches!(
            skill_entry_source(&s),
            ConfigSource::Project { .. }
        ));

        let s = skill_fixture("b", "/repo/.grok/skills/b/SKILL.md", SkillScope::Repo);
        assert!(matches!(
            skill_entry_source(&s),
            ConfigSource::Project { .. }
        ));

        let s = skill_fixture("c", "/home/u/.grok/skills/c/SKILL.md", SkillScope::User);
        assert!(matches!(skill_entry_source(&s), ConfigSource::User { .. }));

        let s = skill_fixture(
            "d",
            "/home/u/.grok/server-skills/d/SKILL.md",
            SkillScope::Server,
        );
        assert!(matches!(
            skill_entry_source(&s),
            ConfigSource::Server { .. }
        ));

        let s = skill_fixture("e", "/home/u/.grok/bundled/e/SKILL.md", SkillScope::Bundled);
        assert!(matches!(
            skill_entry_source(&s),
            ConfigSource::Bundled { .. }
        ));
    }

    /// A discovery-stamped `config_source` (plugins, `[skills].paths`) wins
    /// over the scope fallback.
    #[test]
    fn skill_entry_source_prefers_stamped_config_source() {
        let mut s = skill_fixture("cfg", "/team/skills/cfg/SKILL.md", SkillScope::User);
        s.config_source = Some(ConfigSource::ConfigToml {
            path: PathBuf::from("/team/skills/cfg/SKILL.md"),
        });
        assert!(matches!(
            skill_entry_source(&s),
            ConfigSource::ConfigToml { .. }
        ));
    }

    /// `list_skills` must honor the `[skills]` table like the runtime does:
    /// `paths` skills appear (with a `configToml` source), `ignore`d skills
    /// are hidden, and `disabled` skills stay listed but flagged.
    #[tokio::test]
    async fn list_skills_honors_skills_config() {
        let write = |dir: &Path, name: &str| {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(
                dir.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: test skill {name}\n---\n\nBody.\n"),
            )
            .unwrap();
        };
        // Test-unique names: discovery also reads this machine's real ~/.grok dirs.
        let extra = tempfile::tempdir().unwrap();
        write(&extra.path().join("inspect-cfg-extra"), "inspect-cfg-extra");
        write(
            &extra.path().join("inspect-cfg-ignored"),
            "inspect-cfg-ignored",
        );

        let cwd = tempfile::tempdir().unwrap();
        let config = SkillsConfig {
            paths: vec![extra.path().to_string_lossy().into_owned()],
            ignore: vec![
                extra
                    .path()
                    .join("inspect-cfg-ignored")
                    .to_string_lossy()
                    .into_owned(),
            ],
            disabled: vec!["inspect-cfg-extra".to_string()],
            ..Default::default()
        };
        let registry = xai_grok_agent::plugins::PluginRegistry::from_discovered(vec![], &[], &[]);

        let entries = list_skills(cwd.path(), &registry, &config).await;

        let extra_entry = entries
            .iter()
            .find(|e| e.name == "inspect-cfg-extra")
            .expect("[skills].paths skill should be listed");
        assert!(
            matches!(extra_entry.source, ConfigSource::ConfigToml { .. }),
            "unexpected source: {:?}",
            extra_entry.source
        );
        assert!(
            extra_entry.disabled,
            "[skills].disabled must flag the entry"
        );
        assert!(
            !entries.iter().any(|e| e.name == "inspect-cfg-ignored"),
            "[skills].ignore must hide the skill"
        );
    }

    #[tokio::test]
    async fn list_skills_emits_quarantined_rows_for_top_level_when_to_use() {
        let cwd = tempfile::tempdir().unwrap();
        let skill_dir = cwd.path().join(".grok/skills/leaky");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: leaky\ndescription: A quarantined skill.\nwhen-to-use: secret-token\n---\n# Leaky\n",
        )
        .unwrap();
        let config = SkillsConfig::default();
        let registry = xai_grok_agent::plugins::PluginRegistry::from_discovered(vec![], &[], &[]);
        let entries = list_skills(cwd.path(), &registry, &config).await;
        let leaky = entries
            .iter()
            .find(|e| e.name == "leaky")
            .expect("quarantined skill must appear");
        assert!(leaky.quarantined);
        assert!(!leaky.user_invocable);
        assert!(leaky.disabled);
        assert!(
            leaky
                .diagnostic_codes
                .iter()
                .any(|c| c.contains("when-to-use") || c.contains("unexpected")),
            "codes only: {:?}",
            leaky.diagnostic_codes
        );
        let json = serde_json::to_string(leaky).unwrap();
        assert!(!json.contains("secret-token"), "{json}");
        assert!(
            !json.contains(cwd.path().to_string_lossy().as_ref()),
            "{json}"
        );
    }

    #[tokio::test]
    async fn inspect_report_json_omits_paths_urls_and_credentials() {
        let cwd = tempfile::tempdir().unwrap();
        let report = build_report(cwd.path()).await;
        let json = serde_json::to_string(&report).unwrap();
        assert!(
            !json.contains("/Users/"),
            "inspect JSON must not emit home paths: {json}"
        );
        assert!(
            !json.contains("://"),
            "inspect JSON must not emit URLs: {json}"
        );
        assert!(
            !json.contains("sk-"),
            "inspect JSON must not emit credentials: {json}"
        );
        assert_eq!(report.cwd, workspace_storage_identity(cwd.path()));
        if let Some(fp) = report
            .retrieval
            .as_ref()
            .and_then(|r| r.fingerprint.as_ref())
        {
            assert!(
                fp.chars().count() <= 12,
                "retrieval JSON fingerprint must be truncated: {fp}"
            );
            assert_ne!(
                fp.chars().count(),
                32,
                "JSON must not emit the full 32-hex snapshot fingerprint"
            );
        }
        let hook_json = serde_json::to_string(&HookEntry {
            event: "PreToolUse".into(),
            hook_type: "command".into(),
            target: "https://hooks.example/v1?token=sk-secret".into(),
            source: ConfigSource::User {
                path: PathBuf::from("/Users/me/.grok/hooks.json"),
            },
            matcher: None,
            vendor: None,
            disabled: false,
            compatibility_status: None,
        })
        .unwrap();
        assert!(!hook_json.contains("://"), "{hook_json}");
        assert!(!hook_json.contains("sk-secret"), "{hook_json}");
        assert!(!hook_json.contains("/Users/"), "{hook_json}");
        assert!(
            !json.contains(cwd.path().to_string_lossy().as_ref()),
            "{json}"
        );
    }

    // ── Provider registry / model catalog / retrieval sections ───────────

    fn cfg_from_toml(toml: &str) -> crate::agent::config::Config {
        crate::agent::config::Config::new_from_toml_cfg(&toml::from_str(toml).unwrap()).unwrap()
    }

    #[test]
    fn provider_registry_rows_are_ordered_and_secret_free() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        std::fs::write(
            home.join("config.toml"),
            r#"
            [model_providers.local-a]
            kind = "custom"
            base_url = "http://127.0.0.1:9001/v1"
            display_name = "Local A"
            env_key = "SECRET_ENV_KEY"

            [model_providers.local-a.extra_headers]
            X-Token = "sk-secret-value"

            [model_providers.local-b]
            kind = "custom"
            base_url = "http://127.0.0.1:9002/v1"
            "#,
        )
        .unwrap();
        crate::provider_registry::runtime_cache::invalidate_for_home(home);

        let report = build_provider_registry(home);
        assert!(report.diagnostic.is_none(), "{:?}", report.diagnostic);
        // Canonical built-ins first, then configured entries in input order.
        let ids: Vec<&str> = report.providers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            &ids[..4],
            &["xai", "openai", "openrouter", "anthropic"],
            "built-in order must be deterministic"
        );
        assert!(ids.contains(&"zai-model-api"));
        let pos_a = ids.iter().position(|i| i == &"local-a").unwrap();
        let pos_b = ids.iter().position(|i| i == &"local-b").unwrap();
        assert!(
            pos_a < pos_b,
            "configured order must follow config input order"
        );

        let row_a = report.providers.iter().find(|p| p.id == "local-a").unwrap();
        let row_b = report.providers.iter().find(|p| p.id == "local-b").unwrap();
        assert_eq!(
            row_a.credential_status,
            CredentialStatus::Missing,
            "a declared-but-unset env key is not a usable environment credential"
        );
        assert_eq!(row_b.credential_status, CredentialStatus::Unavailable);
        assert!(
            row_a
                .api_surfaces
                .contains(&"openai_compatible_subset".to_owned())
        );
        assert!(
            row_b.credential_routes.is_empty()
                || row_b.credential_routes.contains(&"none".to_owned())
        );

        // Redaction: never serialize env names, header values, or base URLs.
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("SECRET_ENV_KEY"), "{json}");
        assert!(!json.contains("sk-secret-value"), "{json}");
        assert!(!json.contains("127.0.0.1"), "{json}");
        assert!(!json.contains("credentialGeneration"), "{json}");
    }

    #[test]
    fn provider_registry_is_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        std::fs::write(
            home.join("config.toml"),
            r#"
            [model_providers.local-a]
            kind = "custom"
            base_url = "http://127.0.0.1:9005/v1"
            "#,
        )
        .unwrap();
        crate::provider_registry::runtime_cache::invalidate_for_home(home);

        let a = build_provider_registry(home);
        let b = build_provider_registry(home);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn cache_validity_mapping_covers_all_authoritative_errors() {
        use crate::provider_registry::CacheValidationError;
        assert_eq!(
            cache_validity(&CacheValidationError::Tombstoned),
            CacheValidity::Tombstoned
        );
        assert_eq!(
            cache_validity(&CacheValidationError::Corrupt("x".into())),
            CacheValidity::Corrupt
        );
        assert_eq!(
            cache_validity(&CacheValidationError::OriginMismatch),
            CacheValidity::Mismatch
        );
        assert_eq!(
            cache_validity(&CacheValidationError::BindingMismatch),
            CacheValidity::Mismatch
        );
        assert_eq!(
            cache_validity(&CacheValidationError::Io("x".into())),
            CacheValidity::Unavailable
        );
    }

    #[test]
    fn retrieval_verdict_distinguishes_disk_from_published() {
        assert_eq!(
            retrieval_verdict(true, false),
            (RetrievalValidity::Disabled, false)
        );
        assert_eq!(
            retrieval_verdict(true, true),
            (RetrievalValidity::Valid, false)
        );
        assert_eq!(
            retrieval_verdict(false, true),
            (RetrievalValidity::Invalid, true)
        );
        assert_eq!(
            retrieval_verdict(false, false),
            (RetrievalValidity::Invalid, false)
        );
    }

    #[test]
    fn model_catalog_overlapping_upstreams_aliases_and_determinism() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = cfg_from_toml(
            r#"
            [model."local-producer-a"]
            model = "radiant-model"
            [model."local-producer-b"]
            model = "radiant-model"
            [model."solo"]
            model = "solo-model"
            "#,
        );

        let build = || model_catalog_from_cfg(&cfg, dir.path());
        let m = build();
        assert!(m.diagnostic.is_none(), "{:?}", m.diagnostic);

        let group = m
            .duplicate_groups
            .iter()
            .find(|g| g.upstream_id == "radiant-model")
            .expect("overlapping upstream must be grouped");
        assert_eq!(
            group.canonical_ids,
            vec![
                "local-producer-a".to_string(),
                "local-producer-b".to_string()
            ],
            "canonical candidates must be sorted"
        );

        let ambiguous = m
            .aliases
            .iter()
            .find(|a| a.input == "radiant-model")
            .expect("overlapping slug must be an ambiguous alias");
        assert_eq!(ambiguous.kind, "ambiguous");
        assert_eq!(
            ambiguous.candidates,
            vec![
                "local-producer-a".to_string(),
                "local-producer-b".to_string()
            ]
        );

        // Neither rows nor aliases may leak api-key/environment/display text.
        let json = serde_json::to_string(&m).unwrap();
        assert!(!json.contains("sk-"), "{json}");
        assert!(!json.contains("baseUrl"), "{json}");

        // Determinism: two builds of the same fixture are byte-identical.
        let b = build();
        assert_eq!(
            serde_json::to_string(&m).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn gated_aliases_are_distinct_from_missing_aliases() {
        let mut aliases = std::collections::BTreeMap::new();
        let mut gated = IndexMap::new();
        gated.insert(
            "home-openai:gpt-4o".to_owned(),
            crate::agent::config::ModelEntry::fallback(
                "gpt-4o",
                &crate::agent::config::EndpointsConfig::default(),
            ),
        );

        insert_gated_aliases(&mut aliases, &gated);

        let alias = aliases
            .get("gpt-4o")
            .expect("gated-only upstream id must remain observable");
        assert_eq!(alias.kind, "gated");
        assert_eq!(alias.canonical_id.as_deref(), Some("home-openai:gpt-4o"));
        assert!(alias.candidates.is_empty());
    }

    #[test]
    fn model_catalog_never_gates_explicit_user_keys() {
        use crate::provider_registry::{MULTI_ACCOUNT_ROLLOUT_ENV, with_multi_account_rollout_env};
        with_multi_account_rollout_env(|| {
            unsafe {
                std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, "0");
            }
            let dir = tempfile::tempdir().unwrap();
            let cfg = cfg_from_toml(
                r#"
                [model."my-local"]
                model = "local-model"
                "#,
            );
            let m = model_catalog_from_cfg(&cfg, dir.path());
            // User-authored explicit keys are never treated as rollout-gated.
            assert!(m.gated_entries.is_empty(), "{:?}", m.gated_entries);
            assert!(m.models.iter().any(|r| r.canonical_id == "my-local"));
        });
    }

    #[test]
    fn retrieval_disabled_on_empty_home() {
        let dir = tempfile::tempdir().unwrap();
        let r = build_retrieval(dir.path(), None);
        assert_eq!(r.validity, RetrievalValidity::Disabled);
        assert!(!r.enabled);
        assert!(r.embedding_models.is_empty());
        assert!(r.profiles.is_empty());
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"validity\":\"disabled\""));
    }

    #[test]
    fn retrieval_valid_report_is_secret_free_and_does_not_invent_memory_pin() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let config = r#"[model_providers.emb]
kind = "custom"
base_url = "http://127.0.0.1:9003/v1"

[model_providers.emb.capabilities]
embeddings = true
rerank = true

[embedding_models.primary]
provider = "emb"
model = "emb-model"
dimensions = 8

[reranker_models.rr]
provider = "emb"
model = "rr-model"

[retrieval_profiles.main]
embedding_models = ["primary"]
reranker_models = ["rr"]
max_results = 5
deadline_ms = 3000

[prime.skills]
enabled = true
retrieval_profile = "main"

[memory]
retrieval_profile = "main""#;
        toml::from_str::<toml::Value>(config).expect("valid retrieval inspect fixture");
        std::fs::write(home.join("config.toml"), config).unwrap();
        crate::provider_registry::runtime_cache::invalidate_for_home(home);

        let r = build_retrieval(home, None);
        assert_eq!(r.validity, RetrievalValidity::Valid, "{:?}", r.diagnostic);
        assert_eq!(r.source, "disk");
        assert!(r.enabled);
        assert_eq!(r.embedding_models.len(), 1);
        assert_eq!(r.reranker_models.len(), 1);
        assert_eq!(r.profiles.len(), 1);
        assert!(r.prime.skills.enabled);
        assert_eq!(r.prime.skills.retrieval_profile.as_deref(), Some("main"));
        assert!(
            r.memory_pin.is_none(),
            "a config-derived route is not a persisted memory-index pin"
        );

        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("127.0.0.1"), "{json}");
        assert!(!json.contains("sk-"), "{json}");
        assert!(!json.contains("originHost"), "{json}");
        assert!(!json.contains("prompt"), "{json}");
        assert!(!json.contains("/Users/"), "{json}");
    }

    #[test]
    fn prime_index_inspect_omits_full_fingerprint_bodies_and_paths() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(home.join("config.toml"), "").unwrap();
        let r = build_retrieval(home, Some(workspace.path()));
        if let Some(idx) = &r.prime_index {
            assert!(
                idx.fingerprint_short.chars().count() <= 12,
                "inspect must truncate fingerprints"
            );
        }
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("vectorValues"), "{json}");
        assert!(!json.contains("description"), "{json}");
        assert!(!json.contains("sk-"), "{json}");
        assert!(!json.contains("http://"), "{json}");
        assert!(
            !json.contains(workspace.path().to_string_lossy().as_ref()),
            "{json}"
        );
        if let Some(fp) = &r.fingerprint {
            assert!(
                fp.chars().count() <= 12,
                "retrieval JSON fingerprint must be truncated: {fp}"
            );
        }
    }

    #[test]
    fn memory_pin_reports_installed_source_after_configured_route_changes() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let workspace = tempfile::tempdir().unwrap();
        let config = r#"
            [model_providers.emb]
            kind = "custom"
            base_url = "http://127.0.0.1:9003/v1"

            [model_providers.emb.capabilities]
            embeddings = true

            [embedding_models.primary]
            provider = "emb"
            model = "new-model"
            dimensions = 8

            [retrieval_profiles.main]
            embedding_models = ["primary"]

            [memory]
            retrieval_profile = "main"
        "#;
        std::fs::write(home.join("config.toml"), config).unwrap();
        crate::provider_registry::runtime_cache::invalidate_for_home(home);

        let storage = crate::session::memory::MemoryStorage::new(
            workspace.path(),
            Some(&home.join("memory")),
        );
        std::fs::create_dir_all(storage.workspace_dir()).unwrap();
        let db_path = storage.workspace_dir().join("index.sqlite");
        let conn = xai_sqlite_journal::JournalMode::for_db_path(&db_path)
            .open(&db_path)
            .unwrap();
        conn.execute_batch(
            "CREATE TABLE meta (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);",
        )
        .unwrap();
        let payload = serde_json::json!({
            "version": 1,
            "source": {
                "provider_instance_id": "emb",
                "incarnation": "installed-incarnation",
                "origin_host": "must-not-be-serialized.example",
                "embedding_path": "/v1/embeddings",
                "protocol": "openai_compatible",
                "model": "installed-model",
                "dimensions": 8,
                "encoding": "float",
                "normalization": "none"
            },
            "document_preparation": {
                "version": "v0",
                "chunker": "markdown",
                "max_chunk_chars": 1600,
                "chunk_overlap_chars": 200
            },
            "vector_schema_version": 1
        })
        .to_string();
        for (key, value) in [
            (
                "vector_fingerprint_hash",
                "0123456789abcdef0123456789abcdef",
            ),
            ("vector_fingerprint", payload.as_str()),
            ("vector_rebuild_pending", ""),
        ] {
            conn.execute(
                "INSERT INTO meta(key, value) VALUES (?1, ?2)",
                rusqlite::params![key, value],
            )
            .unwrap();
        }
        drop(conn);

        let report = build_retrieval(home, Some(workspace.path()));
        let pin = report.memory_pin.expect("installed memory pin");
        assert_eq!(pin.installed_provider_instance_id, "emb");
        assert_eq!(pin.installed_model, "installed-model");
        assert_eq!(pin.installed_protocol, "openai_compatible");
        assert!(!pin.configured_route_matches_installed);
        let json = serde_json::to_string(&pin).unwrap();
        assert!(!json.contains("new-model"), "{json}");
        assert!(!json.contains("must-not-be-serialized"), "{json}");

        let conn = xai_sqlite_journal::JournalMode::for_db_path(&db_path)
            .open(&db_path)
            .unwrap();
        conn.execute(
            "UPDATE meta SET value = ?1 WHERE key = 'vector_fingerprint_hash'",
            ["zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"],
        )
        .unwrap();
        drop(conn);
        assert!(
            build_retrieval(home, Some(workspace.path()))
                .memory_pin
                .is_none(),
            "malformed persisted metadata must not become inspect output"
        );
    }

    #[test]
    fn retrieval_invalid_graph_reports_failure_path() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        // Embedding route references a provider that is not configured at all.
        std::fs::write(
            home.join("config.toml"),
            r#"
            [embedding_models.primary]
            provider = "nope"
            model = "emb-model"

            [retrieval_profiles.main]
            embedding_models = ["primary"]
            "#,
        )
        .unwrap();
        crate::provider_registry::runtime_cache::invalidate_for_home(home);

        let r = build_retrieval(home, None);
        // A one-shot inspect has no live LKG, but still distinguishes an
        // invalid disk candidate from deliberately disabled retrieval.
        assert_eq!(r.validity, RetrievalValidity::Invalid);
        assert_eq!(r.source, "disk");
        assert!(r.diagnostic.is_some());
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("sk-"), "{json}");
    }
}
