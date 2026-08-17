#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    unreachable_code,
    dead_code
)]
#[cfg(all(test, feature = "dhat-heap"))]
#[global_allocator]
static DHAT_ALLOC: dhat::Alloc = dhat::Alloc;
pub(crate) use xai_grok_telemetry::unified_log;
pub use xai_tracing_macros::{teprintln, timed, tprintln};
pub mod active_sessions;
pub mod agent;
pub mod auth;
pub mod builtin;
pub mod bundle;
pub mod claude_import;
pub mod claude_import_state;
/// Typed provider / OpenAI / OpenRouter CLI command trees.
pub mod cli;
pub mod cli_models;
pub mod config;
/// Ignored/manual hosted and remote conformance harnesses (Z.ai, solaris).
pub mod conformance;
/// Dynamic multi-provider registry, secrets, caches, and TOML lifecycle.
pub mod provider_registry;
/// Shell-owned retrieval registry, pipeline, and hot-reload (PR17).
pub mod retrieval;
/// Named retrieval graph: parse, validate, management, and notify (PR15).
pub mod retrieval_config;
pub use xai_grok_shell_base::cpu_profile;
pub use xai_grok_shell_base::env;
pub mod extensions;
pub use xai_grok_workspace::foreign_sessions;
pub mod heap_profile;
pub use xai_grok_http as http;
pub mod inspect;
pub mod instrumentation;
pub mod leader;
pub mod managed_config;
pub mod mcp_doctor;
pub use xai_grok_models as models;
pub mod inference;
pub mod plugin;
pub mod relay;
pub mod remote;
pub mod session;
pub mod terminal;
#[cfg(test)]
pub(crate) mod test_support;
pub mod tier;
pub mod tools;
pub mod trace_classifier;
pub mod upload;
pub mod util;

/// Register out-of-tree tool packs (Archanjo, …) for this process.
///
/// Composition roots and tests that build a tool registry must call this
/// before the first `ToolRegistryBuilder::new()`. Idempotent.
pub fn register_extension_tool_packs() {
    archanjo::register();
}
