//! Provider adapter factory — one interface per provider kind.
//!
//! This is **Phase 1** of the Provider Adapter Factory refactor: it defines
//! the provider seam (the object-safe [`ProviderAdapter`] port), the
//! provider-specific data types, one adapter implementation per provider
//! kind, and a factory that produces an adapter from a kind + dialect.
//!
//! ## Nothing is wired yet
//!
//! This module is deliberately self-contained. No call site in the sampler
//! (`client.rs`, the stream codecs, the actor) consumes the adapter yet, so
//! adding it cannot change existing behavior. It exists so later phases can
//! route request construction, reasoning echo, delta shaping, backend
//! negotiation, and error classification through one shared interface
//! instead of duplicating provider branches across the crate.
//!
//! ## Two ports + one collaborator
//!
//! Per the reviewed plan, ONE port is provider-shaped and ONE port is
//! protocol-shaped:
//!
//! - [`ProviderAdapter`] (this module) — identity + request + policy, one
//!   implementation per provider kind.
//! - The **wire codec** (`stream/chat_completions.rs`, `stream/responses.rs`,
//!   `stream/messages.rs`) — protocol-shaped; owns SSE framing and typed
//!   parse. It is NOT part of this module.
//! - The **backend negotiator** (`provider/negotiation.rs`, Phase 6a) — a
//!   concrete collaborator consulted alongside retry's 404
//!   classification, never a port. It is gated behind an off-by-default
//!   `provider.negotiate_backends` flag, so the default runtime path is
//!   byte-identical to pre-negotiation behavior.
//!
//! The shape is intentionally sync + object-safe (no generics, no `Self`, no
//! `async fn`, no free-lifetime RPIT) so a single `Arc<dyn ProviderAdapter>`
//! can be shared lock-free across concurrent requests.

use crate::config::{OriginClientInfo, ProviderIdentity};
use crate::route_context::RouteProviderKind;
use crate::shared_http::ProviderPoolTuning;
use xai_grok_inference_types::{ApiBackend, ChatChunkDelta};

pub mod anthropic;
pub mod compatible;
pub mod dialect;
pub mod negotiation;
pub mod openai;
pub mod openrouter;
pub mod xai;
pub mod zai;

pub use dialect::WireDialect;

/// The provider kind a [`ProviderAdapter`] serves.
///
/// This is the adapter-plane kind (one implementation per variant), distinct
/// from [`RouteProviderKind`] which is the route-partition identifier. The
/// factory maps every feed-in kind (including [`ProviderKind::Custom`], the
/// legacy compatible-family default) to a concrete adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProviderKind {
    /// First-party xAI grok.
    Xai,
    /// OpenAI (API key or ChatGPT subscription OAuth / HTTP Responses).
    OpenAi,
    /// OpenAI-compatible third-party backend (vLLM, SGLang, proxy, etc.).
    OpenAiCompatible,
    /// Direct Anthropic Messages API (`x-api-key` + `anthropic-version`).
    Anthropic,
    /// OpenRouter.
    OpenRouter,
    /// Z.ai Model API.
    Zai,
    /// Unknown/legacy kind. Maps to the openai-compatible standard wire.
    Custom,
}

impl ProviderKind {
    /// Canonical string identifier for this provider kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Xai => "xai",
            Self::OpenAi => "openai",
            Self::OpenAiCompatible => "openai_compatible",
            Self::Anthropic => "anthropic",
            Self::OpenRouter => "openrouter",
            Self::Zai => "zai",
            Self::Custom => "custom",
        }
    }

    /// Parses a canonical string back into a [`ProviderKind`].
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "xai" => Some(Self::Xai),
            "openai" => Some(Self::OpenAi),
            "openai_compatible" => Some(Self::OpenAiCompatible),
            "anthropic" => Some(Self::Anthropic),
            "openrouter" => Some(Self::OpenRouter),
            "zai" => Some(Self::Zai),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    /// Whether this kind is the first-party xAI provider.
    pub const fn is_first_party(self) -> bool {
        matches!(self, Self::Xai)
    }
}

/// Maps a route-partition kind onto the adapter plane. Ambiguous kinds fold
/// to the compatible family, mirroring today's identity mapping:
/// `OpenAiCompatible` and `Zai` both land on `Custom` at the
/// [`ProviderIdentity`] layer today, so `Custom` (and hence
/// `OpenAiCompatible`) is the compatible-family default.
impl From<RouteProviderKind> for ProviderKind {
    fn from(kind: RouteProviderKind) -> Self {
        match kind {
            RouteProviderKind::Xai => Self::Xai,
            RouteProviderKind::OpenAi => Self::OpenAi,
            RouteProviderKind::OpenRouter => Self::OpenRouter,
            RouteProviderKind::Anthropic => Self::Anthropic,
            RouteProviderKind::Zai => Self::Zai,
            RouteProviderKind::OpenAiCompatible | RouteProviderKind::Custom => {
                Self::OpenAiCompatible
            }
        }
    }
}

/// Maps a config-level identity onto the adapter plane. The identity layer
/// is the adapter factory's input (Phase 3a wires it through at
/// `InferenceClient` construction), so every identity has a concrete
/// adapter kind. Unlike [`From<RouteProviderKind> for ProviderKind`], this
/// preserves the legacy `Custom` spelling (which the route-context layer
/// still distinguishes from `OpenAiCompatible`).
impl From<ProviderIdentity> for ProviderKind {
    fn from(identity: ProviderIdentity) -> Self {
        match identity {
            ProviderIdentity::Xai => Self::Xai,
            ProviderIdentity::OpenAi => Self::OpenAi,
            ProviderIdentity::OpenRouter => Self::OpenRouter,
            ProviderIdentity::Anthropic => Self::Anthropic,
            ProviderIdentity::Zai => Self::Zai,
            // `Custom` is the legacy compatible-family default but remains a
            // distinct adapter-plane kind so the route-context mapping can
            // keep the historical `Custom` route partition.
            ProviderIdentity::Custom => Self::Custom,
        }
    }
}

/// Inverse of the route-plane fold: maps an adapter-plane kind back onto the
/// route partition. This is the behavior-preserving direction (Custom stays
/// Custom, OpenAiCompatible stays OpenAiCompatible), distinct from the
/// fold-up [`From<RouteProviderKind> for ProviderKind`].
impl From<ProviderKind> for RouteProviderKind {
    fn from(kind: ProviderKind) -> Self {
        match kind {
            ProviderKind::Xai => Self::Xai,
            ProviderKind::OpenAi => Self::OpenAi,
            ProviderKind::OpenRouter => Self::OpenRouter,
            ProviderKind::Anthropic => Self::Anthropic,
            ProviderKind::Zai => Self::Zai,
            ProviderKind::OpenAiCompatible => Self::OpenAiCompatible,
            ProviderKind::Custom => Self::Custom,
        }
    }
}

/// Credential scheme a provider expects on the wire.
///
/// Distinct from the existing [`crate::config::AuthScheme`] in one way:
/// the adapter port surfaces xAI's first-party token as its own variant so
/// providers are never confused for a plain API-key bearer. (The adapter is
/// not wired into credential resolution yet; this type describes the scheme
/// the provider wants.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthScheme {
    /// `Authorization: Bearer <token>` / `x-api-key` (OpenAI, OpenRouter,
    /// OpenAI-compatible, Z.ai).
    Bearer,
    /// `x-api-key` + `anthropic-version` (direct Anthropic only).
    XApiKey,
    /// First-party xAI session/identity token — never confused for a plain
    /// third-party bearer.
    FirstPartyToken,
}

/// Per-provider usage/echo policy.
///
/// These flags are transcribed verbatim from today's branch sites:
/// - [`Self::include_message_model_id`]: xAI allows the non-standard
///   `messages[].model_id` metadata; every OpenAI-compatible third party
///   (OpenRouter included) rejects it with HTTP 400, so they set `false`.
/// - [`Self::first_party`]: only first-party xAI requests carry stable
///   session/conversation identifiers in `x-grok-*` request headers.
/// - [`Self::openrouter_metadata`]: OpenRouter returns upstream provider
///   diagnostics only when the `X-OpenRouter-Metadata: enabled` header is
///   set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsagePolicy {
    /// Allow `messages[].model_id` metadata on the wire.
    pub include_message_model_id: bool,
    /// First-party xAI (`x-grok-*` headers).
    pub first_party: bool,
    /// OpenRouter diagnostics metadata header requested.
    pub openrouter_metadata: bool,
}

impl UsagePolicy {
    const fn new(include_message_model_id: bool, first_party: bool, openrouter_metadata: bool) -> Self {
        Self {
            include_message_model_id,
            first_party,
            openrouter_metadata,
        }
    }
}

/// Per-provider request pacing default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacingDefault {
    /// No request pacing — requests pass through immediately.
    None,
    /// OpenRouter-style spacing with a default minimum interval (ms) and a
    /// conservative-recovery request count after a 429.
    OpenRouter {
        /// Default minimum interval between requests, in milliseconds.
        min_interval_ms: u64,
        /// Number of successful requests that leave recovery mode.
        recovery_requests: u32,
    },
}

/// A `&'static` bundle describing one provider kind's routing preferences.
///
/// One deep-copy-free lookup per request (the adapter's `policy()` returns a
/// borrowed `'static` reference), bundling the route kind, the preferred
/// backends (unfiltered), the usage policy, and the pacing default. The
/// fields are intentionally `Copy`-friendly so the policy can also live in a
/// `const`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderPolicy {
    /// Route-partition kind this provider maps to.
    pub route_kind: RouteProviderKind,
    /// Preferred backends in priority order (1..=3 entries, unfiltered).
    /// The client is responsible for filtering by its model caps into a
    /// fixed window; this module never inspects model capability.
    pub backend_preference: &'static [ApiBackend],
    /// Per-provider usage/echo policy.
    pub usage_policy: UsagePolicy,
    /// Request pacing default.
    pub pacing_default: PacingDefault,
}

/// Per-request context handed to [`ProviderAdapter::request_extensions`].
///
/// Carries the fields the provider boundary inspects to decide which
/// request-side headers/body geometry to add. Deliberately a cheap snapshot;
/// the adapter may allocate only what it needs.
#[derive(Debug, Clone, Default)]
pub struct RequestContext {
    /// Model being sampled.
    pub model: String,
    /// Base URL of the endpoint.
    pub base_url: String,
    /// Request/response protocol shape.
    pub api_backend: ApiBackend,
    /// Config-level provider identity (already resolved).
    pub provider_identity: ProviderIdentity,
    /// First-party client identity fields (xAI only).
    pub client_version: Option<String>,
    pub deployment_id: Option<String>,
    pub user_id: Option<String>,
    pub client_identifier: Option<String>,
    pub origin_client: Option<OriginClientInfo>,
    /// Extra request headers applied verbatim.
    pub extra_headers: std::collections::BTreeMap<String, String>,
    /// OpenRouter fallback models (request-body extension).
    pub openrouter_fallback_models: Vec<String>,
    /// OpenRouter native `provider` preferences.
    pub openrouter_provider_preferences: Option<crate::config::OpenRouterProviderPreferences>,
    /// OpenRouter native `plugins` array.
    pub openrouter_plugins: Vec<crate::config::OpenRouterPlugin>,
    /// Positive pacing opt-in for non-OpenRouter proxies.
    pub openrouter_pacing: bool,
    /// Z.ai tool-stream flag.
    pub zai_tool_stream: bool,
    /// Z.ai thinking object.
    pub zai_thinking: Option<serde_json::Value>,
    /// Whether `messages[].model_id` metadata is allowed.
    pub include_message_model_id: bool,
}

/// What a provider adds to the request, derived from the context.
///
/// Booleans describe whether a family of request-side additions applies;
/// later phases consume this to actually build headers/body. Phase 1 only
/// produces the description (and tests assert it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestExtensions {
    /// First-party `x-grok-*` client identity headers (xAI only).
    pub first_party_headers: bool,
    /// OpenRouter diagnostics metadata header requested.
    pub openrouter_metadata_header: bool,
    /// OpenRouter request-body extensions (provider/plugins/reasoning).
    pub openrouter_body: bool,
    /// Z.ai request-body extensions (tool_stream / thinking).
    pub zai_body: bool,
    /// ChatGPT subscription OAuth (Codex) wire headers.
    pub chatgpt_oauth_headers: bool,
    /// Allow `messages[].model_id` metadata on the wire.
    pub include_message_model_id: bool,
}

impl Default for RequestExtensions {
    fn default() -> Self {
        Self {
            first_party_headers: false,
            openrouter_metadata_header: false,
            openrouter_body: false,
            zai_body: false,
            chatgpt_oauth_headers: false,
            include_message_model_id: false,
        }
    }
}

/// How reasoning is handled on the request side (keys + echo policy).
///
/// The wire codec uses this for two things: the request-side reasoning key
/// (what key to emit/read for reasoning on the **request**) and the echo
/// policy for replayed assistant reasoning. This deliberately excludes any
/// response decoding — that is the codec's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReasoningWire {
    /// Request-side wire key for reasoning text (e.g. `"reasoning_content"`,
    /// `"reasoning"`, `"thinking"`).
    pub request_key: &'static str,
    /// Echo policy for replayed assistant reasoning.
    pub echo: ReasoningEcho,
}

/// Echo policy for replayed assistant reasoning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningEcho {
    /// Strip reasoning from replayed assistants (vLLM/SGLang dialect —
    /// context-budget / overflow protection).
    Strip,
    /// Echo reasoning content verbatim (default OpenAI-compatible shape).
    Echo,
    /// Echo reasoning content and keep `reasoning_details` blocks
    /// (OpenRouter multi-turn reasoning fidelity).
    Details,
}

/// A provider-neutral HTTP-style error for [`ProviderAdapter::classify_error`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    /// HTTP status code (0 when unknown).
    pub status: u16,
    /// Decoded provider message.
    pub message: String,
}

impl ApiError {
    /// Construct a [`ApiError`] from a status + message.
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

/// Provider-neutral error classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// 401 — permanent auth failure.
    PermanentAuth,
    /// 403 — permanent permission denial.
    PermanentPermission,
    /// 400 / 413 / 422 — actionable client error.
    PermanentActionable,
    /// 404 — endpoint/model not found (often a wrong model id, but can mean
    /// an unsupported backend).
    NotFound,
    /// 429 — retryable per rate-limit policy.
    RetryableRateLimit,
    /// 529 — retryable overload.
    RetryableOverload,
    /// Transient 5xx / network / transport.
    Transient,
    /// Anything else / unknown.
    Other,
}

impl ErrorClass {
    /// Whether this class is retryable (rate limit, overload, transient).
    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::RetryableRateLimit | Self::RetryableOverload | Self::Transient
        )
    }
}

/// Default rate-limit retry threshold for non-OpenRouter providers.
pub const RATE_LIMIT_RETRY_THRESHOLD: u32 = crate::retry::RATE_LIMIT_RETRY_THRESHOLD;

/// Default 429 retry cap for OpenRouter (higher than the generic threshold;
/// overridable via `GROK_OPENROUTER_RATE_LIMIT_RETRIES`).
pub const OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD: u32 =
    crate::retry::OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD;

/// Jackson `DEFAULT_RECOVERY_REQUESTS` pacing knob for OpenRouter.
const OPENROUTER_DEFAULT_MIN_INTERVAL_MS: u64 = 2_000;
const OPENROUTER_DEFAULT_RECOVERY_REQUESTS: u32 = 8;

/// A stateless, object-safe provider seam.
///
/// - Object-safe: no generics, no `Self: Sized`, no `async fn`, no
///   free-lifetime RPIT, so it can be shared as `Arc<dyn ProviderAdapter>`.
/// - All-sync: request construction and policy reads are cheap; the async
///   parts of sampling live in the wire codecs and the actor.
///
/// Every method ships a default body (a no-op or a conservative default) so
/// a future seventh kind does not churn all six implementations when the
/// port grows.
pub trait ProviderAdapter: Send + Sync + 'static {
    /// The provider kind this adapter serves.
    fn id(&self) -> ProviderKind {
        ProviderKind::Custom
    }

    /// The credential scheme this provider expects.
    fn auth(&self) -> AuthScheme {
        AuthScheme::Bearer
    }

    /// A `&'static` routing/usage policy bundle.
    fn policy(&self) -> &'static ProviderPolicy {
        &DEFAULT_PROVIDER_POLICY
    }

    /// Per-request body/header additions derived from the context.
    fn request_extensions(&self, _cx: &RequestContext) -> RequestExtensions {
        RequestExtensions::default()
    }

    /// Request-side reasoning keys + echo policy (never response decode).
    fn reasoning_wire(&self) -> ReasoningWire {
        ReasoningWire {
            request_key: "reasoning_content",
            echo: ReasoningEcho::Echo,
        }
    }

    /// Resolve the per-provider 429 retry cap, honoring an env override where
    /// the provider supports one.
    fn rate_limit_threshold(&self, env: Option<&str>) -> u32 {
        // Non-OpenRouter providers ignore the env override.
        let _ = env;
        RATE_LIMIT_RETRY_THRESHOLD
    }

    /// Classify a provider-neutral error.
    fn classify_error(&self, raw: &ApiError) -> ErrorClass {
        classify_error_by_status(raw.status)
    }

    /// Whether a served model id means the provider transparently failed over
    /// from the requested model. Default: never (only OpenRouter does this).
    fn detect_fallback(&self, requested: &str, served: &str) -> bool {
        let _ = (requested, served);
        false
    }

    /// Optional typed per-delta hook. Default no-op; called once per delta in
    /// the chat_completions codec. Borrows, never allocates. vLLM hoists
    /// `reasoning`; OpenRouter gates `reasoning_details`.
    fn shape_delta(&self, _delta: &mut ChatChunkDelta) {}

    /// Per-provider HTTP pool tuning for **sampling** (Phase 6b / R5).
    ///
    /// The default is the empty [`ProviderPoolTuning`], which keeps sampling
    /// on the single process-wide shared client (zero behavior change). An
    /// adapter may return a non-default tuning (custom pool sizing, connect
    /// timeout, or `http1_only`) to route this provider's sampling through
    /// its own [`ProviderPoolKey`] — giving it an independent connection
    /// pool that multiplexes its own HTTP/2 stream set.
    fn pool_tuning(&self) -> ProviderPoolTuning {
        ProviderPoolTuning::default()
    }
}

/// The fallback policy returned by the trait default.
///
/// `Custom` keeps the legacy standard wire: one backend, no first-party
/// headers, no OpenRouter metadata, no pacing.
pub static DEFAULT_PROVIDER_POLICY: ProviderPolicy = ProviderPolicy {
    route_kind: RouteProviderKind::Custom,
    backend_preference: &[ApiBackend::ChatCompletions],
    usage_policy: UsagePolicy::new(false, false, false),
    pacing_default: PacingDefault::None,
};

/// Classify by status code the way the generic sampler retry path does.
pub(crate) fn classify_error_by_status(status: u16) -> ErrorClass {
    match status {
        401 => ErrorClass::PermanentAuth,
        403 => ErrorClass::PermanentPermission,
        404 => ErrorClass::NotFound,
        400 | 413 | 422 => ErrorClass::PermanentActionable,
        429 => ErrorClass::RetryableRateLimit,
        529 => ErrorClass::RetryableOverload,
        s if (500..600).contains(&s) => ErrorClass::Transient,
        _ => ErrorClass::Other,
    }
}

/// OpenRouter pacing defaults (2 s minimum, 8 recovery requests).
pub(crate) const OPENROUTER_DEFAULT_PACING: PacingDefault = PacingDefault::OpenRouter {
    min_interval_ms: OPENROUTER_DEFAULT_MIN_INTERVAL_MS,
    recovery_requests: OPENROUTER_DEFAULT_RECOVERY_REQUESTS,
};

/// Produces an adapter for a provider kind + wire dialect.
///
/// The match is exhaustive over [`ProviderKind`], so adding a kind is a
/// compile-time audit of every producer site. [`ProviderKind::Custom`] (and
/// any unknown kind) maps to the legacy openai-compatible standard wire.
pub struct ProviderFactory;

impl ProviderFactory {
    /// Build a boxed adapter for `kind` and `dialect`.
    ///
    /// `dialect` is only consulted for [`ProviderKind::OpenAiCompatible`];
    /// every other kind ignores it (it drives no behavior there).
    pub fn build(kind: ProviderKind, dialect: WireDialect) -> Box<dyn ProviderAdapter> {
        match kind {
            ProviderKind::Xai => Box::new(xai::XaiAdapter::new()),
            ProviderKind::OpenAi => Box::new(openai::OpenAiAdapter::new()),
            ProviderKind::OpenAiCompatible => {
                Box::new(compatible::OpenAiCompatibleAdapter::new(dialect))
            }
            ProviderKind::Anthropic => Box::new(anthropic::AnthropicAdapter::new()),
            ProviderKind::OpenRouter => Box::new(openrouter::OpenRouterAdapter::new()),
            ProviderKind::Zai => Box::new(zai::ZaiAdapter::new()),
            // Unknown/legacy kind → the compatible-family default. The wire
            // dialect is honored (not forced to `Standard`): a `Custom`
            // identity with `dialect = "vllm"` must select the vLLM wire so
            // the reasoning-echo `Strip` policy actually applies. When the
            // dialect is `Standard` this is byte-identical to the historical
            // "Custom" behavior.
            ProviderKind::Custom => {
                Box::new(compatible::OpenAiCompatibleAdapter::new(dialect))
            }
        }
    }

    /// Build an adapter from a route-partition kind, mapping onto the adapter
    /// plane (Custom → OpenAiCompatible/Standard).
    pub fn build_for_route_kind(kind: RouteProviderKind) -> Box<dyn ProviderAdapter> {
        Self::build(ProviderKind::from(kind), WireDialect::Standard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_error_by_status_matches_sampler_semantics() {
        assert_eq!(classify_error_by_status(400), ErrorClass::PermanentActionable);
        assert_eq!(classify_error_by_status(401), ErrorClass::PermanentAuth);
        assert_eq!(classify_error_by_status(403), ErrorClass::PermanentPermission);
        assert_eq!(classify_error_by_status(404), ErrorClass::NotFound);
        assert_eq!(classify_error_by_status(413), ErrorClass::PermanentActionable);
        assert_eq!(classify_error_by_status(422), ErrorClass::PermanentActionable);
        assert_eq!(classify_error_by_status(429), ErrorClass::RetryableRateLimit);
        assert_eq!(classify_error_by_status(500), ErrorClass::Transient);
        assert_eq!(classify_error_by_status(529), ErrorClass::RetryableOverload);
        assert_eq!(classify_error_by_status(200), ErrorClass::Other);
    }

    #[test]
    fn route_kind_maps_to_adapter_plane() {
        assert_eq!(
            ProviderKind::from(RouteProviderKind::Xai),
            ProviderKind::Xai
        );
        assert_eq!(
            ProviderKind::from(RouteProviderKind::OpenAi),
            ProviderKind::OpenAi
        );
        assert_eq!(
            ProviderKind::from(RouteProviderKind::OpenRouter),
            ProviderKind::OpenRouter
        );
        assert_eq!(
            ProviderKind::from(RouteProviderKind::Anthropic),
            ProviderKind::Anthropic
        );
        assert_eq!(ProviderKind::from(RouteProviderKind::Zai), ProviderKind::Zai);
        // OpenAiCompatible and Custom both fold to the compatible family.
        assert_eq!(
            ProviderKind::from(RouteProviderKind::OpenAiCompatible),
            ProviderKind::OpenAiCompatible
        );
        assert_eq!(
            ProviderKind::from(RouteProviderKind::Custom),
            ProviderKind::OpenAiCompatible
        );
    }

    #[test]
    fn factory_build_kind_reflected_by_id() {
        let cases: &[(ProviderKind, ProviderKind, WireDialect)] = &[
            (ProviderKind::Xai, ProviderKind::Xai, WireDialect::Standard),
            (ProviderKind::OpenAi, ProviderKind::OpenAi, WireDialect::Standard),
            (
                ProviderKind::OpenAiCompatible,
                ProviderKind::OpenAiCompatible,
                WireDialect::Vllm,
            ),
            (
                ProviderKind::Anthropic,
                ProviderKind::Anthropic,
                WireDialect::Standard,
            ),
            (
                ProviderKind::OpenRouter,
                ProviderKind::OpenRouter,
                WireDialect::Standard,
            ),
            (ProviderKind::Zai, ProviderKind::Zai, WireDialect::Standard),
            // Custom maps to the compatible family.
            (
                ProviderKind::Custom,
                ProviderKind::OpenAiCompatible,
                WireDialect::Standard,
            ),
        ];
        for (input, expected_kind, dialect) in cases {
            let adapter = ProviderFactory::build(*input, *dialect);
            assert_eq!(adapter.id(), *expected_kind, "input {input:?}");
        }
    }

    /// Every production adapter keeps the empty pool tuning by default, so
    /// sampling stays on the single shared client (zero behavior change).
    /// A provider gets its own sampling pool only when it opts into a
    /// non-default tuning (or registers one via `configure_provider_pool_tuning`).
    #[test]
    fn all_adapters_default_to_empty_pool_tuning() {
        let adapters = [
            ProviderFactory::build(ProviderKind::Xai, WireDialect::Standard),
            ProviderFactory::build(ProviderKind::OpenAi, WireDialect::Standard),
            ProviderFactory::build(
                ProviderKind::OpenAiCompatible,
                WireDialect::Standard,
            ),
            ProviderFactory::build(ProviderKind::OpenAiCompatible, WireDialect::Vllm),
            ProviderFactory::build(ProviderKind::Anthropic, WireDialect::Standard),
            ProviderFactory::build(ProviderKind::OpenRouter, WireDialect::Standard),
            ProviderFactory::build(ProviderKind::Zai, WireDialect::Standard),
        ];
        for a in &adapters {
            assert_eq!(
                a.pool_tuning(),
                super::ProviderPoolTuning::default(),
                "{:?} must keep empty pool tuning",
                a.id()
            );
        }
    }

    #[test]
    fn all_six_adapters_policy_and_auth_invariants() {
        let adapters = [
            ProviderFactory::build(ProviderKind::Xai, WireDialect::Standard),
            ProviderFactory::build(ProviderKind::OpenAi, WireDialect::Standard),
            ProviderFactory::build(
                ProviderKind::OpenAiCompatible,
                WireDialect::Standard,
            ),
            ProviderFactory::build(ProviderKind::OpenAiCompatible, WireDialect::Vllm),
            ProviderFactory::build(ProviderKind::Anthropic, WireDialect::Standard),
            ProviderFactory::build(ProviderKind::OpenRouter, WireDialect::Standard),
            ProviderFactory::build(ProviderKind::Zai, WireDialect::Standard),
        ];

        // backend_preference is a 1..=3 static list for every adapter.
        for a in &adapters {
            let policy = a.policy();
            assert!(
                !policy.backend_preference.is_empty() && policy.backend_preference.len() <= 3,
                "backend_preference must be 1..=3 for {:?}",
                a.id()
            );
        }

        // Auth scheme: XApiKey only for Anthropic; FirstPartyToken only for xAI.
        let raw: Vec<ProviderKind> = adapters.iter().map(|a| a.id()).collect();
        for (i, a) in adapters.iter().enumerate() {
            match raw[i] {
                ProviderKind::Xai => assert_eq!(a.auth(), AuthScheme::FirstPartyToken),
                ProviderKind::Anthropic => assert_eq!(a.auth(), AuthScheme::XApiKey),
                _ => assert_eq!(a.auth(), AuthScheme::Bearer),
            }
        }

        // Usage policy flags are gated by kind.
        for a in &adapters {
            let policy = a.policy();
            let is_xai = a.id() == ProviderKind::Xai;
            let is_openrouter = a.id() == ProviderKind::OpenRouter;
            assert_eq!(policy.usage_policy.first_party, is_xai, "{:?}", a.id());
            assert_eq!(
                policy.usage_policy.openrouter_metadata,
                is_openrouter,
                "{:?}",
                a.id()
            );
            assert_eq!(
                policy.usage_policy.include_message_model_id,
                is_xai,
                "{:?}",
                a.id()
            );
        }

        // Fallback detection is OpenRouter-only.
        for a in &adapters {
            assert_eq!(
                a.detect_fallback("requested-model", "served-model"),
                a.id() == ProviderKind::OpenRouter,
                "{:?}",
                a.id()
            );
        }

        // Reasoning echo: Strip only for the vLLM-compatible adapter, Details
        // only for OpenRouter, Echo everywhere else.
        for (i, a) in adapters.iter().enumerate() {
            let expected = match raw[i] {
                ProviderKind::Xai
                | ProviderKind::OpenAi
                | ProviderKind::Anthropic
                | ProviderKind::Zai => ReasoningEcho::Echo,
                ProviderKind::OpenRouter => ReasoningEcho::Details,
                // The vLLM-compatible adapter (index 3) strips; the standard
                // compatible adapter (index 2) echoes.
                ProviderKind::OpenAiCompatible if i == 3 => ReasoningEcho::Strip,
                ProviderKind::OpenAiCompatible => ReasoningEcho::Echo,
                _ => ReasoningEcho::Echo,
            };
            assert_eq!(a.reasoning_wire().echo, expected, "adapter {i}");
        }
    }

    #[test]
    fn all_six_delta_shaping_table() {
        use xai_grok_inference_types::ChatChunkDelta;

        let make_delta = || ChatChunkDelta {
            content: Some("answer".into()),
            reasoning_content: Some("step 1".into()),
            reasoning_details: vec![serde_json::json!({"type": "reasoning.text"})],
            ..ChatChunkDelta::default()
        };

        // Standard compatible clears the OpenRouter-only details, keeps content/reasoning.
        let mut standard = make_delta();
        ProviderFactory::build(ProviderKind::OpenAiCompatible, WireDialect::Standard)
            .shape_delta(&mut standard);
        assert_eq!(standard.content.as_deref(), Some("answer"));
        assert_eq!(standard.reasoning_content.as_deref(), Some("step 1"));
        assert!(standard.reasoning_details.is_empty());

        // vLLM hoists content into the reasoning channel and clears details.
        let mut vllm = make_delta();
        ProviderFactory::build(ProviderKind::OpenAiCompatible, WireDialect::Vllm)
            .shape_delta(&mut vllm);
        assert_eq!(vllm.content, None);
        assert_eq!(vllm.reasoning_content.as_deref(), Some("step 1 answer"));
        assert!(vllm.reasoning_details.is_empty());

        // OpenRouter gates the structured detail blocks through.
        let mut or = make_delta();
        ProviderFactory::build(ProviderKind::OpenRouter, WireDialect::Standard)
            .shape_delta(&mut or);
        assert_eq!(or.reasoning_details.len(), 1);
        assert_eq!(or.content.as_deref(), Some("answer"));

        // Every other provider clears the OpenRouter-only details.
        for kind in [
            ProviderKind::Xai,
            ProviderKind::OpenAi,
            ProviderKind::Anthropic,
            ProviderKind::Zai,
        ] {
            let mut d = make_delta();
            ProviderFactory::build(kind, WireDialect::Standard).shape_delta(&mut d);
            assert!(d.reasoning_details.is_empty(), "kind {kind:?}");
        }
    }
}
