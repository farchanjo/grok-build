// `McpOAuthConfig` / `McpOAuthConfigMap` re-exported via `mcp` (see `mcp.rs`).

mod campaigns;
mod hints;
mod load;
mod mcp;
mod permissions;
mod persist;
mod resolve;
mod settings_writes;
mod tersify;
mod tips;
mod worktree;

pub use campaigns::{
    load_effective_config, load_effective_config_disk_only, merge_project_memory_layer,
    persist_models_default, remote_campaigns_from_settings, set_remote_campaigns_from_settings,
    sync_campaign_fields,
};
pub use hints::*;
pub use load::*;
pub use mcp::*;
pub use permissions::*;
pub use persist::*;
// `remote` extracted to the `xai-grok-config-types` crate (dependency inversion);
// re-exported so `crate::util::config::{RemoteSettings, GoalRoleModel}` keep working.
pub use resolve::*;
pub use settings_writes::*;
pub use tersify::*;
pub use tips::*;
pub use worktree::*;
pub use xai_grok_config_types::{
    CampaignOverride, ContextualHintsRemote, DisplayRefreshSettings, DoomLoopRecoverySettings,
    GoalRoleModel, RemoteSettings, WorktreeAutoGcSettings, WorktreeKindMaxAge,
};

/// `[assets]` + `[assets_providers.<id>]` from the effective config.
///
/// A malformed section degrades to defaults instead of failing session
/// construction: the `asset_*` tools stay usable against the local backend.
pub fn assets_settings_from_effective_config() -> xai_grok_config_types::AssetsSettings {
    load_effective_config()
        .ok()
        .and_then(|root| root.try_into().ok())
        .unwrap_or_default()
}
