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
pub mod attribution;
pub mod client;
pub mod commands;
pub mod config;
pub mod doom_loop;
pub mod events;
pub mod handle;
pub mod metrics;
pub mod retry;
pub mod inference_log;
mod shared_http;
pub mod stream;
pub mod types;

// Public re-exports — the API surface consumers see.
pub use actor::InferenceActor;
pub use attribution::{
    Auth401AttributionCallback, SENT_BEARER_PREFIX_LEN, InferenceConsumer, SharedAttributionCallback,
};
pub use client::{ApiBackend, InferenceClient, user_agent_string_for};
pub use config::{
    AuthScheme, BearerResolver, HeaderInjector, OpenRouterMaxPrice, OpenRouterPlugin,
    OpenRouterProviderPreferences, OriginClientInfo, RetryPolicy, InferenceConfig,
    SharedBearerResolver, SharedHeaderInjector,
};
pub use doom_loop::DoomLoopSignalCollector;
pub use events::{InferenceChannel, InferenceErrorInfo, InferenceErrorKind, InferenceEvent};
pub use handle::InferenceHandle;
pub use metrics::{InferenceLatencyStats, compute_percentiles};
pub use retry::{
    DEFAULT_MAX_RETRIES, RATE_LIMIT_RETRY_THRESHOLD, RetryDecision, classify_error,
    format_inference_error, resolve_max_retries, retry_backoff_with_jitter,
};
pub use inference_log::AuthInfo;
pub use stream::{collect_response, stream_chat_completions, stream_messages, stream_responses};
pub use types::RequestId;
