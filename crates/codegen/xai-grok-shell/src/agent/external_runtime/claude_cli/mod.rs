//! Claude Agent CLI process runtime (PR6 MVP + PR7 advanced).
//!
//! Subscription-backed foreign agent only. Grok hosts UI, process lifecycle,
//! outer sandbox, permission broker, and the normalized envelope — the
//! official `claude` executable owns auth, tools, and the inner agent loop.
//!
//! Compiled only when the `claude-cli-runtime` feature is enabled. Runtime
//! opt-in ([`gates`]) is still required. Never passes API keys. Never claims
//! native Grok tool semantics for Claude-owned tools.

pub mod argv;
pub mod auth;
pub mod capability_mode;
pub mod discovery;
pub mod env_scrub;
pub mod gates;
pub mod mcp_config;
pub mod permission_bridge;
pub mod persistent;
pub mod process;
pub mod protocol;
pub mod provider_status;
pub mod resume_guard;
pub mod runtime;
pub mod sandbox_probe;

pub use argv::{ClaudeCliArgvPlan, ClaudeCliTurnArgv};
pub use capability_mode::ClaudeCapabilityMode;
pub use discovery::{
    ClaudeCliDiscovery, ClaudeCliDiscoveryError, ClaudeCliProbeResult, MIN_CLAUDE_CLI_VERSION,
    discover_claude_executable, probe_claude_version,
};
pub use gates::{
    CLAUDE_CLI_ENV_OPT_IN, claude_cli_both_gates_open, claude_cli_feature_compiled,
    claude_cli_runtime_opt_in, parse_runtime_opt_in_value,
};
pub use permission_bridge::{
    PERMISSION_BRIDGE_SUBCOMMAND, maybe_run_permission_bridge_subprocess,
    permission_prompt_tool_flag,
};
pub use runtime::{ClaudeCliRuntime, ClaudeCliRuntimeFactory};

/// UI label under the Anthropic provider card.
pub const CLAUDE_CLI_UI_LABEL: &str = "Claude Agent (CLI, Experimental)";

/// Longer limitations blurb for pickers / status.
pub const CLAUDE_CLI_UI_LIMITATIONS: &str = "\
Experimental subscription-backed Claude Agent CLI. Claude owns auth and tools; \
Grok owns the permission broker, outer process/sandbox, and UI. No Grok tool \
loop, compaction, memory, goals, hooks, checkpoints, or workflow accounting. \
No API keys. No bypassPermissions. Session-scoped runtime reuse across turns; \
persistent multi-turn child when the binary advertises streaming input, \
otherwise one process per turn on the retained runtime.";
