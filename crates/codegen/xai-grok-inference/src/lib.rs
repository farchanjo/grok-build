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
pub mod handle;
pub mod inference_log;
pub mod metrics;
/// Reusable OpenAI / OpenRouter platform client (Changes 7–13).
pub mod openai_platform;
pub mod openrouter_baseline;
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
    Auth401AttributionCallback, InferenceConsumer, SENT_BEARER_PREFIX_LEN,
    SharedAttributionCallback,
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
pub use events::{InferenceChannel, InferenceErrorInfo, InferenceErrorKind, InferenceEvent};
pub use handle::InferenceHandle;
pub use inference_log::AuthInfo;
pub use metrics::{InferenceLatencyStats, compute_percentiles};
pub use openai_platform::{
    OPERATION_BINDINGS, OpenAiAdminClient, OpenAiClient, OpenRouterClient, PlatformClientConfig,
    PlatformError, PlatformResult, PlatformTransport, TOTAL_BINDING_COUNT, TransportPolicy,
    assert_zero_uncovered_operations, coverage_report_json,
};
pub use route_context::{
    ProviderRouteContext, ProviderRouteContextBuilder, RouteApiSurface, RouteAuthority,
    RouteContextUpdate, RouteCredentialRoute, RoutePacingOverride, RouteProviderKind,
};
pub use openrouter_baseline::{
    OpenRouterEndpoint, OpenRouterEndpointInventory, coding_agent_priority_endpoints,
    inventory_has_endpoint, openrouter_endpoint_inventory, schema_field_names,
};
pub use retry::{
    DEFAULT_MAX_RETRIES, RATE_LIMIT_RETRY_THRESHOLD, RetryDecision, classify_error,
    format_inference_error, resolve_max_retries, retry_backoff_with_jitter,
};
pub use stream::{collect_response, stream_chat_completions, stream_messages, stream_responses};
pub use types::RequestId;
