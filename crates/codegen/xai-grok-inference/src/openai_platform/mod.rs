//! Provider-neutral OpenAI platform client facade.
//!
//! Application and administration clients share transport policy while keeping
//! credentials structurally isolated. OpenRouter-native operations live under a
//! separate client and binding namespace.
//!
//! Generated operation methods cover every endpoint in the pinned OpenAI and
//! OpenRouter baseline inventories (see `generated/bindings.rs`).

pub mod client;
pub mod error;
pub mod generated;
pub mod inventory_coverage;
pub mod realtime;
pub mod transport;
pub mod types;
pub mod url_policy;

#[cfg(test)]
mod transport_tests;

pub use client::{OpenAiAdminClient, OpenAiClient, OpenRouterClient, PlatformClientConfig};
pub use error::{PlatformError, PlatformResult};
pub use generated::{
    BINARY_PRIMARY_COUNT, OPENAI_ADMIN_BINDING_COUNT, OPENAI_APP_BINDING_COUNT,
    OPENAI_PRIMARY_COUNT, OPENROUTER_BINDING_COUNT, OPENROUTER_PRIMARY_COUNT, OPERATION_BINDINGS,
    OperationBinding, SSE_COMPANION_COUNT, TOTAL_BINDING_COUNT, find_binding,
    openrouter_path_is_admin, operation_requires_confirmation,
};
pub use inventory_coverage::{
    assert_zero_uncovered_operations, coverage_report_json, uncovered_operations,
};
pub use realtime::{RealtimeClientEvent, RealtimeEventPayload, RealtimeServerEvent};
pub use transport::{
    CredentialKind, HttpRequestSpec, MultipartFiles, PlatformTransport, RealtimeSession, SseEvent,
    TransportPolicy,
};
pub use types::{DeleteStatus, JsonObject, ListQuery, PathParams, PlatformPage};
