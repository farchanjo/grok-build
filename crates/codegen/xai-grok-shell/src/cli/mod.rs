//! Typed CLI facades for provider lifecycle and platform operations.

pub mod generated_dispatch;
pub mod generated_ops;
pub mod instance_dispatch;
pub mod openai_cmd;
pub mod openrouter_cmd;
pub mod output;
pub mod provider_cmd;
pub mod typed_dispatch_runtime;

pub use generated_ops::{
    CLI_OPERATION_COUNT, CLI_OPERATIONS, CliOperation, find_cli_operation,
    operation_requires_confirmation,
};
pub use instance_dispatch::{
    OPENAI_COMPATIBLE_SUBSET_OPERATION_IDS, SelectedInstance, assert_surface_allows_operation,
    dry_run_document, load_provider_service, resolve_selected_instance,
};
pub use openai_cmd::{OpenAiCliArgs, OpenAiCliCommand, run_openai_cli};
pub use openrouter_cmd::{OpenRouterCliArgs, OpenRouterCliCommand, run_openrouter_cli};
pub use provider_cmd::{
    ProviderLifecycleArgs, ProviderLifecycleCommand, run_provider_lifecycle_cli,
};
