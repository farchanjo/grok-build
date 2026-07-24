//! Archanjo — out-of-tree custom tool pack for Grok Build.
//!
//! Modular registration (hexagonal composition):
//!
//! 1. Tools live here, not in `xai-grok-tools` product core.
//! 2. Ports (injected resources) are defined next to the tool that needs them.
//! 3. The shell/adapters inject backends at session rebuild time.
//! 4. The composition root (`xai-grok-pager-bin`) calls [`register`] once
//!    before any `ToolRegistryBuilder::new()`.
//!
//! ## Tools
//!
//! - [`SearchModelsTool`] — catalog name → slug lookup for subagent spawn.

#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod search_models;

pub use search_models::{
    ModelCatalogQuery, ModelCatalogSearch, SearchModelsHit, SearchModelsInput, SearchModelsResult,
    SearchModelsTool,
};

use std::sync::Once;

use xai_grok_tools::registry::types::{ToolPack, ToolRegistryBuilder, register_tool_pack};

/// Register every Archanjo tool with the process-global tool-pack registry.
///
/// Safe to call multiple times; only the first call takes effect. Must run
/// before the first `ToolRegistryBuilder::new()` in the process (composition
/// root responsibility).
pub fn register() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        register_tool_pack(ARCHANJO_TOOL_PACK);
        tracing::debug!("archanjo tool pack registered");
    });
}

const ARCHANJO_TOOL_PACK: ToolPack = |builder: &mut ToolRegistryBuilder| {
    builder.register::<SearchModelsTool>();
};
