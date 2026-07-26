//! Sampler configuration types.
//!
//! [`InferenceConfig`] is the per-request configuration handed to the
//! sampler. It deliberately does **not** alias
//! `xai_grok_inference_types::InferenceSettings` so that the sampler crate
//! avoids transitive dependencies on shell-specific types
//! (`xai-grok-tools`, etc.).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use xai_grok_inference_types::{
    ApiBackend, CompactionAtTokens, CompactionsRemaining, DoomLoopRecoveryPolicy, ReasoningEffort,
};

use crate::attribution::SharedAttributionCallback;
use crate::retry::{DEFAULT_MAX_RETRIES, RATE_LIMIT_RETRY_THRESHOLD};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthScheme {
    #[default]
    Bearer,
    XApiKey,
}

/// Which upstream provider a `InferenceConfig` targets.
///
/// The shell derives this from `ModelProviderKind` when building a
/// `InferenceConfig`. The sampler uses it to decide whether to attach
/// first-party `x-grok-*` request headers (`Xai` only) and whether to
/// treat OpenRouter diagnostics metadata as requested (`OpenRouter`
/// only). The default is `Custom` — the safest choice for an unknown
/// provider, because no first-party headers are sent and no
/// OpenRouter-specific diagnostics path is taken.
///
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderIdentity {
    #[default]
    Custom,
    #[serde(rename = "xai")]
    Xai,
    /// OpenAI API key or ChatGPT subscription OAuth (HTTP Responses).
    #[serde(rename = "openai", alias = "codex")]
    OpenAi,
    #[serde(rename = "openrouter")]
    OpenRouter,
    /// Direct Anthropic Messages API (`x-api-key` + `anthropic-version`).
    /// Never first-party xAI and never OpenRouter routing/diagnostics.
    #[serde(rename = "anthropic")]
    Anthropic,
}

/// OpenRouter routing `sort`: string shorthand (`"latency"`) or object form
/// (`{ "by": "latency", ... }`) per the pinned OpenAPI `ProviderPreferences`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OpenRouterSort {
    /// Documented string form: `"price"`, `"throughput"`, or `"latency"`.
    Name(String),
    /// Object form with a primary `by` key and additive vendor fields.
    Object {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<String>,
        #[serde(flatten)]
        extra: indexmap::IndexMap<String, serde_json::Value>,
    },
}

impl OpenRouterSort {
    /// String-form constructor used by tests and simple TOML configs.
    pub fn name(value: impl Into<String>) -> Self {
        Self::Name(value.into())
    }

    /// Object-form constructor with a required `by` dimension.
    pub fn by(value: impl Into<String>) -> Self {
        Self::Object {
            by: Some(value.into()),
            extra: indexmap::IndexMap::new(),
        }
    }

    /// Primary sort dimension when represented as a string or `{ by }`.
    pub fn as_name(&self) -> Option<&str> {
        match self {
            Self::Name(s) => Some(s.as_str()),
            Self::Object { by, .. } => by.as_deref(),
        }
    }
}

/// OpenRouter's native `provider` request-body object. All fields are
/// optional; OpenRouter applies its own defaults when a field is absent. The
/// sampler omits the entire `provider` key when no preferences are configured,
/// and omits individual fields when they are `None` or empty so the wire body
/// never carries an empty object/array (some upstreams reject those).
///
/// Owned by the sampler crate (like [`ProviderIdentity`]) so both the shell
/// TOML layer and the sampler wire layer share one type.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenRouterProviderPreferences {
    /// Routing sort: string (`"latency"`) or object (`{ "by": "latency" }`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<OpenRouterSort>,
    /// Preferred provider slugs, in descending priority.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// Provider slugs to use exclusively.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub only: Vec<String>,
    /// Provider slugs to skip.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<String>,
    /// Allow fallbacks to other providers when the primary fails.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    /// Only use providers supporting the request's parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    /// `"allow"` or `"deny"` — whether providers may train on the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<String>,
    /// Zero-data retention override (opt-in).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zdr: Option<bool>,
    /// Quantization preferences (e.g. `["int8"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quantizations: Vec<String>,
    /// Maximum price caps per token kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_price: Option<OpenRouterMaxPrice>,
    /// Prefer providers that serve distillable text (OpenAPI field).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforce_distillable_text: Option<bool>,
    /// Preferred maximum latency budget. OpenAPI allows number or object;
    /// kept as JSON so both shapes round-trip without schema lag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_max_latency: Option<serde_json::Value>,
    /// Preferred minimum throughput. OpenAPI allows number or object.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_min_throughput: Option<serde_json::Value>,
}

/// Normalized OpenRouter Chat `reasoning` object (effort + optional knobs).
/// Distinct from flat `reasoning_effort` so OpenRouter receives the documented
/// object form while other providers keep OpenAI-compatible flat effort.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpenRouterReasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<bool>,
}

impl OpenRouterReasoning {
    pub fn from_effort(effort: xai_grok_inference_types::ReasoningEffort) -> Self {
        Self {
            effort: Some(effort.as_str().to_owned()),
            max_tokens: None,
            exclude: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.effort.is_none() && self.max_tokens.is_none() && self.exclude.is_none()
    }
}

/// Per-kind price cap for [`OpenRouterProviderPreferences::max_price`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenRouterMaxPrice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<f64>,
}

/// OpenRouter native `plugins` array entry. The documented wire shape is a
/// table with a required `id` and arbitrary provider-specific knobs (e.g.
/// `{ id = "web", max_results = 3 }`). We keep a light type — `id` plus a
/// flattened `extra` map — so shape drift in the extra fields is tolerated
/// without modeling each one field-by-field.
///
/// Owned by the sampler crate (like [`ProviderIdentity`]) so both the shell
/// TOML layer and the sampler wire layer share one definition.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpenRouterPlugin {
    /// Plugin identifier (e.g. `"response-healing"`, `"web"`).
    pub id: String,
    /// Flattened provider-specific options. Serialized inline (not nested
    /// under an `extra` key) so the wire shape matches
    /// `{ "id": "web", "max_results": 3 }`. Unknown keys round-trip verbatim.
    #[serde(flatten)]
    pub extra: indexmap::IndexMap<String, serde_json::Value>,
}

impl OpenRouterProviderPreferences {
    /// Returns `true` when every field is unset, so the `provider` key can be
    /// omitted entirely from the request body.
    pub fn is_empty(&self) -> bool {
        self.sort.is_none()
            && self.order.is_empty()
            && self.only.is_empty()
            && self.ignore.is_empty()
            && self.allow_fallbacks.is_none()
            && self.require_parameters.is_none()
            && self.data_collection.is_none()
            && self.zdr.is_none()
            && self.quantizations.is_empty()
            && self
                .max_price
                .as_ref()
                .map(|m| m.prompt.is_none() && m.completion.is_none())
                .unwrap_or(true)
            && self.enforce_distillable_text.is_none()
            && self.preferred_max_latency.is_none()
            && self.preferred_min_throughput.is_none()
    }
}

impl ProviderIdentity {
    /// Returns `true` only for the first-party xAI provider.
    ///
    /// Only first-party xAI requests carry the stable session/conversation
    /// identifiers in `x-grok-*` request headers. Third-party providers
    /// (OpenAI, OpenRouter, Anthropic, custom) must never see those headers.
    pub fn is_first_party(self) -> bool {
        matches!(self, ProviderIdentity::Xai)
    }

    /// Returns `true` only for OpenRouter, where upstream provider
    /// diagnostics metadata is explicitly requested via the
    /// `X-OpenRouter-Metadata` header set by the shell.
    pub fn is_openrouter(self) -> bool {
        matches!(self, ProviderIdentity::OpenRouter)
    }

    /// Returns `true` only for direct Anthropic (not OpenRouter Claude routes
    /// and not custom Messages backends).
    pub fn is_anthropic(self) -> bool {
        matches!(self, ProviderIdentity::Anthropic)
    }

    /// Generic user-facing label for this provider, used in
    /// provider-aware error copy (502/520-class, 402, etc.). This is the
    /// fallback when the diagnostics `provider_name` (the selected
    /// OpenRouter upstream) is unavailable at the call site.
    ///
    /// xAI keeps the historical "Grok" wording; OpenRouter, OpenAI, and
    /// Anthropic use their product names; `Custom` uses a neutral phrase.
    pub fn label(self) -> &'static str {
        match self {
            ProviderIdentity::Xai => "Grok",
            ProviderIdentity::OpenAi => "OpenAI",
            ProviderIdentity::OpenRouter => "OpenRouter",
            ProviderIdentity::Anthropic => "Anthropic",
            ProviderIdentity::Custom => "the model provider",
        }
    }
}

/// All knobs that control a single sampling request.
///
/// The session typically owns one `InferenceConfig` per active model
/// and passes it (or a per-request override) to the actor on every
/// submit.
///
/// # Construction in `xai-grok-shell`
///
/// `InferenceConfig` is the single source of truth for sampler
/// configuration. The shell builds it directly (see
/// `agent::config::inference_config_for_model` and
/// `session::acp_session::SessionActor::reconstruct_full_config`) by
/// composing chat-state's `xai_grok_inference_types::InferenceSettings`
/// with `Credentials` (api key, client version).
///
/// URL-derived request headers (e.g. `X-XAI-Token-Auth` for the
/// cli-chat-proxy) are
/// folded into [`Self::extra_headers`] by
/// `agent::config::inject_url_derived_headers` before the
/// `InferenceConfig` is handed to the actor. Auth is selected separately
/// via `auth_scheme`, while `api_backend` controls only the request/response
/// protocol shape.
///
/// # Credential safety
///
/// [`Self::api_key`] is never written to `Debug` output. Serde **serialization**
/// omits the field entirely (`skip_serializing`) so diagnostics, IPC dumps,
/// and accidental `serde_json::to_value` cannot leak the secret. Deserialization
/// still accepts `api_key` so in-process and test round-trips that re-attach a
/// key remain valid; production callers resolve the key from the vault/env and
/// set the field in memory only.
#[derive(Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// In-memory API credential. Omitted from serde serialization and Debug.
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub max_completion_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    /// OpenRouter model fallbacks, tried in order after [`Self::model`].
    ///
    /// This is an OpenRouter-only request-body extension. An empty list is
    /// omitted from the wire, preserving the standard OpenAI-compatible
    /// request body for every other provider.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openrouter_fallback_models: Vec<String>,
    /// OpenRouter native `provider` request-body preferences. Only serialized
    /// when the identity is OpenRouter and the object is non-empty; `None` or
    /// an all-empty object omits the `provider` key entirely.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openrouter_provider_preferences: Option<OpenRouterProviderPreferences>,
    /// OpenRouter native `plugins` request-body array. Only serialized when
    /// the identity is OpenRouter and the list is non-empty; an empty list
    /// omits the `plugins` key entirely. Never emitted for non-OpenRouter
    /// providers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openrouter_plugins: Vec<OpenRouterPlugin>,
    /// Explicit OpenRouter request pacing opt-in for OpenRouter-compatible
    /// proxies that keep a non-`OpenRouter` [`ProviderIdentity`]. Built-in
    /// OpenRouter identity always paces; hostname `openrouter.ai` remains a
    /// legacy fallback. Extensions (provider/plugins/reasoning object) stay
    /// gated on identity only.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub openrouter_pacing: bool,
    /// Z.ai Model API: enable fragmented tool argument streaming (`tool_stream`).
    /// Only serialized for Z.ai-profiled configs; never for other providers.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub zai_tool_stream: bool,
    /// Z.ai Model API: thinking object (`thinking.type` / `clear_thinking`).
    /// Only serialized when present and the Z.ai profile is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zai_thinking: Option<serde_json::Value>,
    pub api_backend: ApiBackend,
    /// Whether Chat Completions history may include xAI's non-standard
    /// `messages[].model_id` metadata. OpenAI-compatible third-party
    /// providers such as OpenRouter reject that field with HTTP 400.
    #[serde(default = "default_include_message_model_id")]
    pub include_message_model_id: bool,
    #[serde(default)]
    pub auth_scheme: AuthScheme,

    /// Which upstream provider this config targets. The shell derives this
    /// from `ModelProviderKind`. The sampler uses it to gate first-party
    /// `x-grok-*` request header injection (`Xai` only) and to decide whether
    /// OpenRouter diagnostics metadata was requested (`OpenRouter` only).
    /// Defaults to `Custom` — the safest choice for an unknown provider.
    #[serde(default)]
    pub provider_identity: ProviderIdentity,
    /// Extra request headers applied verbatim. The sampler never inspects
    /// the URL to derive headers; callers (the session) inject proxy auth
    /// and other access headers here before constructing the config.
    pub extra_headers: IndexMap<String, String>,
    /// Total context window size in tokens. The sampler does not enforce
    /// it; it is informational metadata used by the session for compaction
    /// decisions.
    pub context_window: u64,
    pub force_http1: bool,
    pub max_retries: Option<u32>,
    pub stream_tool_calls: bool,
    pub idle_timeout_secs: Option<u64>,

    // Reasoning effort
    pub reasoning_effort: Option<ReasoningEffort>,

    // Client identity
    pub origin_client: Option<OriginClientInfo>,
    pub client_identifier: Option<String>,
    pub deployment_id: Option<String>,
    pub user_id: Option<String>,
    pub client_version: Option<String>,

    /// Optional hook invoked at every UNAUTHORIZED (401) response
    /// site. The sampler passes the bearer that was actually sent on
    /// the wire to the callback; the implementation is free to do
    /// whatever it wants with it (typically: join it with a live
    /// credential source and emit an attribution event for diagnosis
    /// of stale-token vs. server-rejected-live-token 401s). `None`
    /// (default) is a no-op -- the 401 arm returns the same
    /// `InferenceError::Auth` it always did.
    ///
    /// `Arc<dyn Trait>` is not serializable, so the field is skipped
    /// in (de)serialization. Round-tripping a config through serde
    /// drops the callback; callers that deserialize a `InferenceConfig`
    /// from disk must re-attach the callback before passing it to
    /// [`crate::InferenceClient::new`] or 401 attribution will be
    /// silently disabled for the rebuilt client.
    #[serde(skip)]
    pub attribution_callback: Option<SharedAttributionCallback>,

    /// Live bearer resolve per request. `None` uses construction-time `api_key`.
    #[serde(skip)]
    pub bearer_resolver: Option<SharedBearerResolver>,

    #[serde(default)]
    pub supports_backend_search: bool,

    /// Per-model config for the `x-compactions-remaining` header; `None` disables it.
    #[serde(default)]
    pub compactions_remaining: Option<CompactionsRemaining>,

    /// Per-model config for the `x-compaction-at` header; `None` disables it.
    #[serde(default)]
    pub compaction_at_tokens: Option<CompactionAtTokens>,

    /// Server-side doom-loop check policy; `None` disables it. When set, the
    /// client itself sends the opt-in `x-grok-doom-loop-check` header on
    /// streaming Responses API requests and absorbs the reported trigger
    /// events (unlike the environment headers in [`Self::extra_headers`],
    /// this header gates the client's own decode behavior, so it lives with
    /// the decoder).
    #[serde(default)]
    pub doom_loop_recovery: Option<DoomLoopRecoveryPolicy>,

    /// Per-request header injector (e.g. OTel traceparent). Called in `post()`.
    #[serde(skip)]
    pub header_injector: Option<SharedHeaderInjector>,
}

impl std::fmt::Debug for InferenceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Deliberately omit `api_key` (and other non-serializable hooks).
        f.debug_struct("InferenceConfig")
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("max_completion_tokens", &self.max_completion_tokens)
            .field("temperature", &self.temperature)
            .field("top_p", &self.top_p)
            .field(
                "openrouter_fallback_models",
                &self.openrouter_fallback_models,
            )
            .field(
                "openrouter_provider_preferences",
                &self.openrouter_provider_preferences,
            )
            .field("openrouter_plugins", &self.openrouter_plugins)
            .field("openrouter_pacing", &self.openrouter_pacing)
            .field("zai_tool_stream", &self.zai_tool_stream)
            .field("zai_thinking", &self.zai_thinking)
            .field("api_backend", &self.api_backend)
            .field("include_message_model_id", &self.include_message_model_id)
            .field("auth_scheme", &self.auth_scheme)
            .field("provider_identity", &self.provider_identity)
            .field("extra_headers", &self.extra_headers)
            .field("context_window", &self.context_window)
            .field("force_http1", &self.force_http1)
            .field("max_retries", &self.max_retries)
            .field("stream_tool_calls", &self.stream_tool_calls)
            .field("idle_timeout_secs", &self.idle_timeout_secs)
            .field("reasoning_effort", &self.reasoning_effort)
            .field("origin_client", &self.origin_client)
            .field("client_identifier", &self.client_identifier)
            .field("deployment_id", &self.deployment_id)
            .field("user_id", &self.user_id)
            .field("client_version", &self.client_version)
            .field(
                "attribution_callback",
                &self.attribution_callback.as_ref().map(|_| "<set>"),
            )
            .field(
                "bearer_resolver",
                &self.bearer_resolver.as_ref().map(|_| "<set>"),
            )
            .field("supports_backend_search", &self.supports_backend_search)
            .field("compactions_remaining", &self.compactions_remaining)
            .field("compaction_at_tokens", &self.compaction_at_tokens)
            .field("doom_loop_recovery", &self.doom_loop_recovery)
            .field(
                "header_injector",
                &self.header_injector.as_ref().map(|_| "<set>"),
            )
            .finish()
    }
}

impl Default for InferenceConfig {
    /// Empty defaults so callers can use `..Default::default()` and
    /// new fields don't ripple through every literal site.
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: String::new(),
            model: String::new(),
            max_completion_tokens: None,
            temperature: None,
            top_p: None,
            openrouter_fallback_models: Vec::new(),
            openrouter_provider_preferences: None,
            openrouter_plugins: Vec::new(),
            openrouter_pacing: false,
            zai_tool_stream: false,
            zai_thinking: None,
            api_backend: ApiBackend::default(),
            include_message_model_id: true,
            auth_scheme: AuthScheme::default(),
            provider_identity: ProviderIdentity::default(),
            extra_headers: IndexMap::new(),
            context_window: 0,
            force_http1: false,
            max_retries: None,
            stream_tool_calls: false,
            idle_timeout_secs: None,
            reasoning_effort: None,
            origin_client: None,
            client_identifier: None,
            deployment_id: None,
            user_id: None,
            client_version: None,
            attribution_callback: None,
            bearer_resolver: None,
            supports_backend_search: false,
            compactions_remaining: None,
            compaction_at_tokens: None,
            doom_loop_recovery: None,
            header_injector: None,
        }
    }
}

const fn default_include_message_model_id() -> bool {
    true
}

/// Cheap sync read of the current bearer for [`InferenceConfig::bearer_resolver`].
pub trait BearerResolver: Send + Sync + std::fmt::Debug {
    fn current_bearer(&self) -> Option<String>;
}

pub type SharedBearerResolver = std::sync::Arc<dyn BearerResolver>;

/// Per-request header injection (e.g. OTel `traceparent`).
pub trait HeaderInjector: Send + Sync + std::fmt::Debug {
    fn inject(&self, headers: &mut reqwest::header::HeaderMap);
}

pub type SharedHeaderInjector = std::sync::Arc<dyn HeaderInjector>;

/// Retry knobs for the sampler's internal transport-error retry loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retries before giving up.
    pub max_retries: u32,
    /// After this many rate-limit (429) retries, escalate to the caller.
    /// Lower than `max_retries` because rate-limit waits can be long.
    pub rate_limit_retry_threshold: u32,
    #[serde(default)]
    pub retry_only_before_output: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            rate_limit_retry_threshold: RATE_LIMIT_RETRY_THRESHOLD,
            retry_only_before_output: false,
        }
    }
}

/// Identity of the client that originated the request, used for
/// User-Agent rendering. The shell layer composes this with platform
/// info into a final UA string.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OriginClientInfo {
    pub product: String,
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_policy_defaults() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(
            policy.rate_limit_retry_threshold,
            RATE_LIMIT_RETRY_THRESHOLD
        );
    }

    /// Configs serialized before the field existed must keep deserializing.
    #[test]
    fn config_without_doom_loop_recovery_deserializes_to_none() {
        let mut stripped = serde_json::to_value(InferenceConfig::default()).unwrap();
        stripped
            .as_object_mut()
            .unwrap()
            .remove("doom_loop_recovery");
        let config: InferenceConfig = serde_json::from_value(stripped).unwrap();
        assert!(config.doom_loop_recovery.is_none());

        let with_policy = InferenceConfig {
            doom_loop_recovery: Some(DoomLoopRecoveryPolicy {
                max_threshold: 8,
                max_retries: 2,
            }),
            ..Default::default()
        };
        let round_tripped: InferenceConfig =
            serde_json::from_value(serde_json::to_value(&with_policy).unwrap()).unwrap();
        assert_eq!(
            round_tripped.doom_loop_recovery,
            with_policy.doom_loop_recovery
        );
    }

    #[test]
    fn legacy_config_keeps_xai_message_metadata_behavior() {
        let mut stripped = serde_json::to_value(InferenceConfig::default()).unwrap();
        stripped
            .as_object_mut()
            .unwrap()
            .remove("include_message_model_id");

        let config: InferenceConfig = serde_json::from_value(stripped).unwrap();
        assert!(config.include_message_model_id);
    }

    /// `provider_identity` defaults to `Custom` (the safest choice: no
    /// first-party headers) and survives serde round-trips.
    #[test]
    fn provider_identity_round_trips_and_defaults_to_custom() {
        // Default is Custom.
        assert_eq!(ProviderIdentity::default(), ProviderIdentity::Custom);
        assert!(!ProviderIdentity::default().is_first_party());
        assert!(!ProviderIdentity::default().is_openrouter());

        // A config missing the field (legacy serialization) deserializes to Custom.
        let mut stripped = serde_json::to_value(InferenceConfig::default()).unwrap();
        stripped
            .as_object_mut()
            .unwrap()
            .remove("provider_identity");
        let config: InferenceConfig = serde_json::from_value(stripped).unwrap();
        assert_eq!(config.provider_identity, ProviderIdentity::Custom);

        // Each variant round-trips through serde.
        for identity in [
            ProviderIdentity::Custom,
            ProviderIdentity::Xai,
            ProviderIdentity::OpenAi,
            ProviderIdentity::OpenRouter,
            ProviderIdentity::Anthropic,
        ] {
            let cfg = InferenceConfig {
                provider_identity: identity,
                ..Default::default()
            };
            let round_tripped: InferenceConfig =
                serde_json::from_value(serde_json::to_value(&cfg).unwrap()).unwrap();
            assert_eq!(round_tripped.provider_identity, identity);
        }
    }

    /// `is_first_party` is true only for `Xai`.
    #[test]
    fn provider_identity_first_party_only_for_xai() {
        assert!(ProviderIdentity::Xai.is_first_party());
        assert!(!ProviderIdentity::OpenAi.is_first_party());
        assert!(!ProviderIdentity::OpenRouter.is_first_party());
        assert!(!ProviderIdentity::Anthropic.is_first_party());
        assert!(!ProviderIdentity::Custom.is_first_party());
    }

    /// `is_openrouter` is true only for `OpenRouter`.
    #[test]
    fn provider_identity_openrouter_only_for_openrouter() {
        assert!(ProviderIdentity::OpenRouter.is_openrouter());
        assert!(!ProviderIdentity::Xai.is_openrouter());
        assert!(!ProviderIdentity::OpenAi.is_openrouter());
        assert!(!ProviderIdentity::Anthropic.is_openrouter());
        assert!(!ProviderIdentity::Custom.is_openrouter());
    }

    /// `is_anthropic` is true only for direct Anthropic.
    #[test]
    fn provider_identity_anthropic_only_for_anthropic() {
        assert!(ProviderIdentity::Anthropic.is_anthropic());
        assert!(!ProviderIdentity::Xai.is_anthropic());
        assert!(!ProviderIdentity::OpenAi.is_anthropic());
        assert!(!ProviderIdentity::OpenRouter.is_anthropic());
        assert!(!ProviderIdentity::Custom.is_anthropic());
    }

    /// Credentials must never appear in Debug or serde serialization for any
    /// provider (not just Anthropic).
    #[test]
    fn inference_config_debug_and_json_never_leak_api_key() {
        const SECRET: &str = "sk-super-secret-api-key-value-never-log";
        let cfg = InferenceConfig {
            api_key: Some(SECRET.to_owned()),
            base_url: "https://api.example.com/v1".into(),
            model: "test-model".into(),
            provider_identity: ProviderIdentity::OpenAi,
            ..Default::default()
        };
        let debug = format!("{cfg:?}");
        assert!(
            !debug.contains(SECRET),
            "Debug must redact api_key: {debug}"
        );
        assert!(
            debug.contains("<redacted>"),
            "Debug should mark the key as redacted: {debug}"
        );
        let json = serde_json::to_value(&cfg).expect("serialize");
        let json_str = json.to_string();
        assert!(
            !json_str.contains(SECRET),
            "JSON serialization must omit raw api_key: {json_str}"
        );
        assert!(
            json.get("api_key").is_none(),
            "api_key field must be skip_serializing: {json_str}"
        );
        // Deserialization still accepts an explicit key when present.
        let with_key = serde_json::json!({
            "api_key": SECRET,
            "base_url": "https://api.example.com/v1",
            "model": "m",
            "api_backend": "chat_completions",
            "extra_headers": {},
            "context_window": 0,
            "force_http1": false,
            "stream_tool_calls": false,
        });
        let loaded: InferenceConfig = serde_json::from_value(with_key).unwrap();
        assert_eq!(loaded.api_key.as_deref(), Some(SECRET));
    }
}
