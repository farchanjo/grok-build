//! Claude Agent CLI process runtime (PR6).
//!
//! Subscription-backed foreign agent only. Grok hosts UI, process lifecycle,
//! outer sandbox, and the normalized envelope — the official `claude`
//! executable owns auth, tools, and the inner agent loop.
//!
//! Compiled only when the `claude-cli-runtime` feature is enabled. Runtime
//! opt-in ([`gates`]) is still required. Never passes API keys.

pub mod argv;
pub mod auth;
pub mod discovery;
pub mod env_scrub;
pub mod gates;
pub mod process;
pub mod protocol;
pub mod runtime;

pub use argv::{ClaudeCliArgvPlan, ClaudeCliTurnArgv};
pub use discovery::{
    ClaudeCliDiscovery, ClaudeCliDiscoveryError, ClaudeCliProbeResult, MIN_CLAUDE_CLI_VERSION,
    discover_claude_executable, probe_claude_version,
};
pub use gates::{
    CLAUDE_CLI_CONFIG_KEY, CLAUDE_CLI_ENV_OPT_IN, claude_cli_both_gates_open,
    claude_cli_feature_compiled, claude_cli_runtime_opt_in, parse_runtime_opt_in_value,
};
pub use runtime::{ClaudeCliRuntime, ClaudeCliRuntimeFactory};

/// UI label under the Anthropic provider card.
pub const CLAUDE_CLI_UI_LABEL: &str = "Claude Agent (CLI, Experimental)";

/// Longer limitations blurb for pickers / status.
pub const CLAUDE_CLI_UI_LIMITATIONS: &str = "\
Experimental subscription-backed Claude Agent CLI. One process per Grok turn; \
Claude owns auth and tools. No Grok tool loop, compaction, memory, goals, or \
workflow. No API keys. Persistent input / permission bridge / MCP deferred.";
