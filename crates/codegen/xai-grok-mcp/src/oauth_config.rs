//! OAuth configuration types for MCP servers.
//!
//! Constructed by the host's TOML parsing (`McpServerConfig::oauth_config`)
//! and consumed by [`crate::oauth`].
//!
//! The definitions live in `xai-grok-config-types` (the config leaf crate) and
//! are re-exported here so the historical path
//! `xai_grok_mcp::oauth_config::McpOAuthConfig` stays valid. Defining them in
//! the leaf keeps `xai-grok-config-types` free of a dependency on this crate,
//! which is what lets `xai-file-utils` depend on it without a cycle.

pub use xai_grok_config_types::{McpOAuthConfig, McpOAuthConfigMap};
