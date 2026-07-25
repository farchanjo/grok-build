pub mod conversation;
pub mod error;
pub mod types;

// `Client` is the legacy alias used throughout the shell. A later refactor
// retired the bespoke shell HTTP client and points `Client` at the sampler crate's
// `InferenceClient` -- the two have identical method sets, so call-sites
// compile unchanged.
pub use self::conversation::*;
pub use self::error::{InferenceError, ResponseModelMetadata, Result};
pub use self::types::*;
pub use xai_grok_inference::ApiBackend;
pub use xai_grok_inference::InferenceClient as Client;

// Re-export async-openai Responses API types under `rs` namespace
pub use async_openai::types::responses as rs;

// ---------------------------------------------------------------------------
// xai-grok-inference re-exports
// ---------------------------------------------------------------------------
//
// The actual streaming / retry / HTTP-client logic lives in the
// `xai-grok-inference` crate. We re-export the public surface here so
// `crate::inference::{InferenceHandle, InferenceConfig, ...}` paths keep working
// for callers that haven't been ported to spell these directly via
// `xai_grok_inference::*`. The shell-side `sampling::client::Config`
// composite was removed when its only remaining role -- session-snapshot
// state for `MvpAgent` -- was migrated to `RefCell<InferenceConfig>` directly.
pub use xai_grok_inference::{
    InferenceActor, InferenceChannel, InferenceClient, InferenceConfig, InferenceErrorInfo,
    InferenceErrorKind, InferenceEvent, InferenceHandle, InferenceLatencyStats, OriginClientInfo,
    RequestId,
};
