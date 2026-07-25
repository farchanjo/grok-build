//! Typed CLI facades for provider lifecycle and platform operations.

pub mod generated_ops;
pub mod openai_cmd;
pub mod openrouter_cmd;
pub mod output;
pub mod provider_cmd;

pub use generated_ops::{CLI_OPERATION_COUNT, CLI_OPERATIONS, CliOperation, find_cli_operation};
pub use openai_cmd::{OpenAiCliArgs, OpenAiCliCommand, run_openai_cli};
pub use openrouter_cmd::{OpenRouterCliArgs, OpenRouterCliCommand, run_openrouter_cli};
pub use provider_cmd::{
    ProviderLifecycleArgs, ProviderLifecycleCommand, run_provider_lifecycle_cli,
};
