//! Pure data types for the xAI inference / chat-completion API layer.
//!
//! This crate contains the API-agnostic conversation types, chat completion
//! request/response types, streaming types, and error types used across the
//! xAI agent stack.  It intentionally contains **no I/O** (no HTTP clients,
//! no file system access) so it can be depended on by downstream crates
//! (e.g., `xai-chat-state`) without pulling in the full `xai-grok-shell`.

pub mod anthropic;
pub mod codex_wire;
pub mod conversation;
pub mod doom_loop;
pub mod error;
pub mod messages;
pub mod serde_helpers;
pub mod tool_overrides;
pub mod types;

pub use self::anthropic::{
    ANTHROPIC_VERSION, AnthropicBeta, AnthropicBetaSet, AnthropicErrorBody, AnthropicErrorObject,
    AnthropicErrorType, AnthropicRateLimitHeaders, CapabilitySupport, CountTokensRequest,
    CountTokensResponse, DeleteFileResponse, EffortCapability, FILES_API_BETA, FileListPage,
    FileMetadata, FileUploadSource, ListFilesParams, ListModelsParams, ModelCapabilities,
    ModelInfo, ModelListPage,
};
pub use self::codex_wire::{
    clear_chatgpt_codex_create_response_fields, is_chatgpt_codex_base_url,
    shape_chatgpt_codex_responses_body,
};
pub use self::conversation::*;
pub use self::doom_loop::{
    DOOM_LOOP_CHECK_EVENT_TYPE, DOOM_LOOP_CHECK_HEADER, DoomLoopPeek, DoomLoopRecoveryPolicy,
    DoomLoopSignal, DoomLoopSignalKind, is_check_event, peek_doom_loop,
};
pub use self::error::{
    ApiErrorDiagnostics, EmptyReason, EmptyResponseContext, InferenceError, ResponseModelMetadata,
    Result, is_context_length_error, status_user_message, status_user_message_for,
    user_facing_api_error_message, user_facing_api_error_message_for,
};
pub use self::tool_overrides::{
    ClearableField, SearchDateBound, SearchDateBoundError, ToolOverrides, ToolOverridesUpdate,
    WebSearchOptions, XSearchOptions,
};
pub use self::types::*;

// Re-export async-openai crate Responses API types under `rs` namespace
pub use async_openai::types::responses as rs;
