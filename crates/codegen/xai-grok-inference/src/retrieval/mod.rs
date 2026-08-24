//! Provider-scoped typed embedding and reranker adapters (PR16).
//!
//! Handwritten retrieval API surface suitable for PR17 clients. Generated
//! OpenAI/OpenRouter files are never edited; OpenRouter rerank calls the
//! generated `OpenRouterClient::create_rerank` entry point.

pub mod base64_f32;
pub mod embeddings;
pub mod openrouter_rerank;
pub mod transport;
pub mod types;
pub mod vllm_rerank;

#[cfg(test)]
mod http_tests;

pub use base64_f32::decode_base64_f32;
pub use embeddings::{OpenaiCompatibleEmbeddings, parse_embedding_response_for_test};
pub use openrouter_rerank::{OpenRouterRerankAdapter, map_openrouter_result};
pub use transport::{RetrievalCredential, RetrievalTransport, RetrievalTransportPolicy};
pub use types::{
    DEFAULT_CONNECT_TIMEOUT, DEFAULT_EMBEDDINGS_PATH, DEFAULT_MAX_REDIRECTS,
    DEFAULT_MAX_RESPONSE_BYTES, DEFAULT_MAX_RETRIES, DEFAULT_REQUEST_TIMEOUT, DEFAULT_RERANK_PATH,
    DEFAULT_TOTAL_DEADLINE, EmbeddingAdapter, EmbeddingEncodingFormat, EmbeddingRequest,
    EmbeddingResult, EmbeddingVector, MAX_EMBEDDING_DIMENSIONS, MAX_EMBEDDING_INPUT_CHARS,
    MAX_EMBEDDING_INPUTS, MAX_RERANK_DOCUMENTS, MAX_RERANK_TEXT_CHARS, MAX_RERANK_TOP_N,
    RerankAdapter, RerankHit, RerankRequest, RerankResult, RetrievalAuthScheme, RetrievalError,
    RetrievalErrorCategory, RetrievalPurpose, RetrievalResult, RetrievalRouteContext,
    normalize_endpoint_path, redact_error_preview, validate_embedding_request,
    validate_relative_endpoint_path, validate_rerank_request,
};
pub use vllm_rerank::{VllmRerankAdapter, parse_vllm_rerank_response};
