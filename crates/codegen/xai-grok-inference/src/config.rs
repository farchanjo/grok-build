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
    /// Routing sort: `"price"`, `"throughput"`, or `"latency"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
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
    }
}

impl ProviderIdentity {
    /// Returns `true` only for the first-party xAI provider.
    ///
    /// Only first-party xAI requests carry the stable session/conversation
    /// identifiers in `x-grok-*` request headers. Third-party providers
    /// (OpenAI, OpenRouter, custom) must never see those headers.
    pub fn is_first_party(self) -> bool {
        matches!(self, ProviderIdentity::Xai)
    }

    /// Returns `true` only for OpenRouter, where upstream provider
    /// diagnostics metadata is explicitly requested via the
    /// `X-OpenRouter-Metadata` header set by the shell.
    pub fn is_openrouter(self) -> bool {
        matches!(self, ProviderIdentity::OpenRouter)
    }

    /// Generic user-facing label for this provider, used in
    /// provider-aware error copy (502/520-class, 402, etc.). This is the
    /// fallback when the diagnostics `provider_name` (the selected
    /// OpenRouter upstream) is unavailable at the call site.
    ///
    /// xAI keeps the historical "Grok" wording; OpenRouter and OpenAI use
    /// their product names; `Custom` uses a neutral phrase.
    pub fn label(self) -> &'static str {
        match self {
            ProviderIdentity::Xai => "Grok",
            ProviderIdentity::OpenAi => "OpenAI",
            ProviderIdentity::OpenRouter => "OpenRouter",
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
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
        assert!(!ProviderIdentity::Custom.is_first_party());
    }

    /// `is_openrouter` is true only for `OpenRouter`.
    #[test]
    fn provider_identity_openrouter_only_for_openrouter() {
        assert!(ProviderIdentity::OpenRouter.is_openrouter());
        assert!(!ProviderIdentity::Xai.is_openrouter());
        assert!(!ProviderIdentity::OpenAi.is_openrouter());
        assert!(!ProviderIdentity::Custom.is_openrouter());
    }
}
