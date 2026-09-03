//! xai-grok-inference - Actor-based inference layer for xAI grok.
//!
//! This crate extracts the HTTP streaming + retry logic out of
//! `xai-grok-shell`'s session actor into a standalone, reusable
//! component built on the same actor pattern as `xai-hunk-tracker`.
//!
//! ## Layered API
//!
//! - **Layer 1**: [`client::InferenceClient`] returns raw chunk streams.
//! - **Layer 2**: [`stream`] transforms raw streams into [`InferenceEvent`]s.
//! - **Layer 3**: [`InferenceHandle`] manages concurrent requests with retry,
//!   cancellation, and event-based coordination via the actor.
//!
//! The type skeleton, the pure retry / metrics / client logic, the
//! Layer-2 stream transforms ([`stream_chat_completions`],
//! [`stream_responses`], [`stream_messages`], [`collect_response`]),
//! and the actor with its per-request task tie these layers together.

pub mod actor;
/// Repository-owned Anthropic HTTP client (direct identity, not Messages protocol).
pub mod anthropic;
pub mod attribution;
pub mod client;
pub mod commands;
/// OpenAI / OpenRouter compatibility baselines and contracts (Change 4).
pub mod compatibility;
pub mod config;
pub mod doom_loop;
pub mod events;
pub mod extra_ca;
pub mod handle;
pub mod inference_log;
pub mod metrics;
/// Reusable OpenAI / OpenRouter platform client (Changes 7–13).
pub mod openai_platform;
pub mod openrouter_baseline;
/// Provider-scoped typed embedding and reranker adapters (PR16).
pub mod retrieval;
pub mod retry;
/// Credential-free provider route context for sampler partitioning and repair.
pub mod route_context;
mod shared_http;
pub mod stream;
pub mod types;

// Public re-exports — the API surface consumers see.
pub use actor::InferenceActor;
pub use anthropic::{
    ANTHROPIC_VERSION, AnthropicBeta, AnthropicBetaSet, AnthropicClient, AnthropicClientConfig,
    AnthropicClientError, AnthropicErrorBody, AnthropicMessagesOutcome, AnthropicPage,
    AnthropicRateLimitHeaders, AnthropicResponseMeta, AnthropicResult, CountTokensRequest,
    CountTokensResponse, DEFAULT_ANTHROPIC_BASE_URL, DeleteFileResponse, ErrorClass,
    FILES_API_BETA, FileListPage, FileMetadata, FileUploadSource, ListFilesParams,
    ListModelsParams, MAX_REQUEST_BYTES, ModelInfo, ModelListPage,
    parse_anthropic_rate_limit_headers,
};
pub use attribution::{
    Auth401AttributionCallback, BEARER_TAIL_CHARS, InferenceConsumer, SharedAttributionCallback,
};
pub use client::{ApiBackend, InferenceClient, user_agent_string_for};
pub use compatibility::{
    BindingStatus, ClaimSurface, CompatibilityCounts, CompatibilityStatus, DeclaredIntersection,
    Evidence, EvidenceKind, OperationClaim, OperationIdentity, ProviderInventory, Transport,
    claim_is_consistent, compatibility_counts, declared_intersection, intersection_report_json,
    inventory_report_json, openai_inventory, openrouter_inventory as openrouter_provider_inventory,
    sha256_hex_is_valid, source_revision_is_valid, timestamp_is_rfc3339_utc,
};
pub use config::{
    AuthScheme, BearerResolver, HeaderInjector, InferenceConfig, OpenRouterMaxPrice,
    OpenRouterPlugin, OpenRouterProviderPreferences, OpenRouterReasoning, OpenRouterSort,
    OriginClientInfo, RetryPolicy, SharedBearerResolver, SharedHeaderInjector,
};
pub use doom_loop::DoomLoopSignalCollector;
pub use events::{
    InferenceChannel, InferenceErrorInfo, InferenceErrorKind, InferenceEvent, StripReason,
};
pub use handle::InferenceHandle;
pub use inference_log::AuthInfo;
pub use metrics::{InferenceLatencyStats, compute_percentiles};
pub use openai_platform::{
    OPERATION_BINDINGS, OpenAiAdminClient, OpenAiClient, OpenRouterClient, PlatformClientConfig,
    PlatformError, PlatformResult, PlatformTransport, TOTAL_BINDING_COUNT, TransportPolicy,
    assert_zero_uncovered_operations, coverage_report_json,
};
pub use openrouter_baseline::{
    OpenRouterEndpoint, OpenRouterEndpointInventory, coding_agent_priority_endpoints,
    inventory_has_endpoint, openrouter_endpoint_inventory, schema_field_names,
};
pub use retrieval::{
    DEFAULT_EMBEDDINGS_PATH, DEFAULT_RERANK_PATH, EmbeddingAdapter, EmbeddingEncodingFormat,
    EmbeddingRequest, EmbeddingResult, EmbeddingVector, OpenRouterRerankAdapter,
    OpenaiCompatibleEmbeddings, RerankAdapter, RerankHit, RerankRequest, RerankResult,
    RetrievalAuthScheme, RetrievalCredential, RetrievalError, RetrievalErrorCategory,
    RetrievalPurpose, RetrievalResult, RetrievalRouteContext, RetrievalTransport,
    RetrievalTransportPolicy, VllmRerankAdapter, decode_base64_f32, normalize_endpoint_path,
    parse_embedding_response_for_test, parse_vllm_rerank_response, validate_embedding_request,
    validate_relative_endpoint_path, validate_rerank_request,
};
// Prefer `xai_grok_inference::retrieval::...` for retrieval-only constants that
// would collide with sampler retry exports (e.g. DEFAULT_MAX_RETRIES).
pub use retry::{
    DEFAULT_MAX_RETRIES, RATE_LIMIT_RETRY_THRESHOLD, RetryDecision, classify_error,
    format_inference_error, resolve_max_retries, retry_backoff_with_jitter,
};
pub use route_context::{
    NormalizedOrigin, ProviderRouteContext, ProviderRouteContextBuilder, RouteApiSurface,
    RouteAuthority, RouteContextUpdate, RouteCredentialRoute, RoutePacingOverride,
    RouteProviderKind,
};
pub use shared_http::{
    ProviderPoolTuning, configure_provider_pool_tuning, effective_provider_connect_timeout,
};
pub use stream::{collect_response, stream_chat_completions, stream_messages, stream_responses};
pub use types::RequestId;
