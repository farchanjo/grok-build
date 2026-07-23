//! System prompt assembly — template rendering, AGENTS.md, and skills.

/// Provider-neutral engineering role shared with native-agent bridges.
///
/// The full Grok Build prompt contains the same contract with additional
/// runtime-specific tool and safety instructions. Native agent providers use
/// this compact form as developer instructions so they do not inherit an
/// incorrect product/model identity from the host executable.
pub const PROVIDER_NEUTRAL_ENGINEERING_INSTRUCTIONS: &str =
    include_str!("../../templates/provider_neutral_engineering.md");

pub mod agents_md;
pub mod context;
pub mod ignore;
pub mod skills;
pub mod subagent_prompts;
pub mod template;
pub mod user_message;
pub mod workspace_user;
