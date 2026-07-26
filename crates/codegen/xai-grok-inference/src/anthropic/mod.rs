//! Repository-owned Anthropic HTTP client.
//!
//! Direct Anthropic identity boundary: always sends `x-api-key` (never
//! `Authorization: Bearer`) and pins `anthropic-version: 2023-06-01`.
//!
//! This is distinct from [`crate::client::InferenceClient`] with
//! [`crate::client::ApiBackend::Messages`], which is a protocol adapter and
//! must not receive Anthropic headers/defaults solely because the protocol
//! is Messages.

mod client;
mod error;
mod headers;

#[cfg(test)]
mod tests;

pub use client::{
    AnthropicClient, AnthropicClientConfig, AnthropicMessagesOutcome, AnthropicPage,
    DEFAULT_ANTHROPIC_BASE_URL, MAX_REQUEST_BYTES,
};
pub use error::{AnthropicClientError, AnthropicResult, ErrorClass};
pub use headers::{AnthropicResponseMeta, parse_anthropic_rate_limit_headers};
pub use xai_grok_inference_types::anthropic::{
    ANTHROPIC_VERSION, AnthropicBeta, AnthropicBetaSet, AnthropicErrorBody, AnthropicErrorObject,
    AnthropicErrorType, AnthropicRateLimitHeaders, CountTokensRequest, CountTokensResponse,
    DeleteFileResponse, FILES_API_BETA, FileListPage, FileMetadata, FileUploadSource,
    ListFilesParams, ListModelsParams, ModelInfo, ModelListPage,
};
