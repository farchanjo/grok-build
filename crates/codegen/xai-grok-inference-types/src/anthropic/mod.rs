//! Anthropic API wire types (no I/O).
//!
//! Messages create/stream types live in [`crate::messages`]. This module holds
//! shared Anthropic constants, betas, errors, models, token counting, files,
//! and rate-limit metadata used by the repository-owned Anthropic client.

pub mod beta;
pub mod count_tokens;
pub mod error;
pub mod files;
pub mod models;
pub mod rate_limit;
pub mod version;

pub use beta::{AnthropicBeta, AnthropicBetaSet, FILES_API_BETA};
pub use count_tokens::{CountTokensRequest, CountTokensResponse};
pub use error::{AnthropicErrorBody, AnthropicErrorObject, AnthropicErrorType};
pub use files::{
    DeleteFileResponse, FileListPage, FileMetadata, FileUploadSource, ListFilesParams,
};
pub use models::{
    CapabilitySupport, ListModelsParams, ModelCapabilities, ModelInfo, ModelListPage,
};
pub use rate_limit::AnthropicRateLimitHeaders;
pub use version::ANTHROPIC_VERSION;
