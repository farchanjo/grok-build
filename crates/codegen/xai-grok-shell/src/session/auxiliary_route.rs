//! Exact provider-route resolution for every auxiliary inference path.
//!
//! Compaction, media understanding, web search, title/summary/recap generation,
//! prompt/shell suggestions, and the goal evaluator all resolve through this
//! module. Explicit overrides are **canonical selection IDs** resolved once via
//! the origin-aware PR4 resolver; only [`UpstreamModelId`] reaches wire
//! requests. `@session` copies the frozen session route (never a live global
//! picker). Missing, ambiguous, namespaced-hijack, or unavailable-credential
//! routes fail closed and never borrow a sibling account. Catalog entries that
//! identity-resolution gates out surface as [`AuxiliaryRouteError::Missing`]
//! (not a distinct gated variant).
//!
//! Compatibility (`legacy_compat`) activates only for catalog-absent,
//! non-namespaced, validated historical first-party wire slugs. The sidecar
//! identity is deliberately non-authoritative (`aux_legacy_compat`,
//! [`RouteAuthority::HostFallback`], [`RouteCredentialRoute::None`], zero
//! generations) so it cannot match exact-account 401 repair.

use std::path::Path;

use xai_grok_inference::{
    InferenceConfig, ProviderRouteContext, RouteApiSurface, RouteAuthority, RouteCredentialRoute,
    RouteProviderKind,
};
use xai_grok_models::{UpstreamModelId, split_first_colon};

use crate::agent::config::{
    ModelEntry, stamp_session_local_sampler_fields, try_inference_config_for_model,
};
use crate::agent::model_identity::{
    ModelIdentityResolution, resolve_model_identity_with_origins, upstream_id_for_entry,
};
use crate::agent::models::ModelsManager;

/// Non-authoritative instance id for historical first-party wire-slug compat.
pub const AUX_LEGACY_COMPAT_INSTANCE: &str = "aux_legacy_compat";

/// Sentinel that inherits the frozen session route.
pub const SESSION_ROUTE_SENTINEL: &str = "@session";

/// Secret-free purpose labels used for disclosure/provenance metadata only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuxiliaryPurpose {
    Compaction,
    CompactionRecap,
    MediaDescribe,
    MediaVideo,
    MediaPdf,
    MediaStt,
    WebSearch,
    SessionTitle,
    SessionRecap,
    PromptSuggest,
    ShellSuggest,
    GoalEvaluator,
    GoalClassifier,
    AutoClassifier,
    /// `/btw` side-question one-shot (inherits frozen session route).
    SideQuestion,
    /// Laziness / stalled-turn classifier one-shot (inherits frozen session).
    LazinessClassifier,
}

impl AuxiliaryPurpose {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compaction => "compaction",
            Self::CompactionRecap => "compaction_recap",
            Self::MediaDescribe => "media_describe",
            Self::MediaVideo => "media_video",
            Self::MediaPdf => "media_pdf",
            Self::MediaStt => "media_stt",
            Self::WebSearch => "web_search",
            Self::SessionTitle => "session_title",
            Self::SessionRecap => "session_recap",
            Self::PromptSuggest => "prompt_suggest",
            Self::ShellSuggest => "shell_suggest",
            Self::GoalEvaluator => "goal_evaluator",
            Self::GoalClassifier => "goal_classifier",
            Self::AutoClassifier => "auto_classifier",
            Self::SideQuestion => "side_question",
            Self::LazinessClassifier => "laziness_classifier",
        }
    }

    /// Operation-partition label for pacing / attribution.
    pub const fn operation_partition(self) -> &'static str {
        self.as_str()
    }
}

/// How the auxiliary route was selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuxiliaryRouteKind {
    /// Inherited the frozen session route (`@session`).
    SessionInherit,
    /// Explicit catalog selection resolved origin-aware.
    Explicit,
    /// Hidden first-party historical wire slug (catalog-absent only).
    LegacyCompat,
}

/// Fail-closed reasons for auxiliary route resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuxiliaryRouteError {
    Missing { input: String },
    Ambiguous { input: String },
    SessionRouteRequired,
    CredentialUnavailable { selection: String },
    ConstructionFailed { selection: String, detail: String },
    ExplicitPinFailed { selection: String },
    NamespacedHijackRejected { input: String },
}

impl std::fmt::Display for AuxiliaryRouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { input } => write!(f, "auxiliary model `{input}` is not in the catalog"),
            Self::Ambiguous { input } => {
                write!(f, "auxiliary model `{input}` is ambiguous across accounts")
            }
            Self::SessionRouteRequired => {
                write!(
                    f,
                    "@session auxiliary route requires a frozen session route"
                )
            }
            Self::CredentialUnavailable { selection } => {
                write!(
                    f,
                    "auxiliary model `{selection}` has no usable credentials on its route"
                )
            }
            Self::ConstructionFailed { selection, detail } => {
                // detail is intentionally free of secrets/URLs/PII.
                write!(
                    f,
                    "auxiliary model `{selection}` could not be initialized ({detail})"
                )
            }
            Self::ExplicitPinFailed { selection } => {
                write!(
                    f,
                    "explicit auxiliary pin `{selection}` could not be resolved"
                )
            }
            Self::NamespacedHijackRejected { input } => {
                write!(
                    f,
                    "namespaced auxiliary id `{input}` failed authoritative resolution"
                )
            }
        }
    }
}

impl std::error::Error for AuxiliaryRouteError {}

/// Exact resolved auxiliary route handle.
#[derive(Clone, Debug)]
pub struct ResolvedAuxiliaryRoute {
    pub purpose: AuxiliaryPurpose,
    pub kind: AuxiliaryRouteKind,
    /// Canonical catalog selection when catalog-backed; absent for pure legacy.
    pub canonical_selection_id: Option<String>,
    /// Wire model id — the only model string that reaches requests.
    pub upstream_model_id: String,
    pub inference: InferenceConfig,
    pub route: ProviderRouteContext,
}

impl ResolvedAuxiliaryRoute {
    /// Whether this resolved route may bind its own exact-instance 401
    /// attribution (telemetry + route-cell comparison).
    ///
    /// Clears only for truly non-authoritative compatibility identities:
    /// `LegacyCompat`, [`RouteAuthority::HostFallback`],
    /// [`RouteCredentialRoute::None`], or an empty / legacy instance id.
    /// Exact BYOK / configured routes that resolve as `Unverified` (no on-disk
    /// binding generation) still retain a route-bound callback when they have
    /// a real credential route and instance id — without broadening
    /// exact-account *repair* beyond what `ProviderRouteContext` authority
    /// already encodes.
    pub fn supports_route_bound_attribution(&self) -> bool {
        use xai_grok_inference::{RouteAuthority, RouteCredentialRoute};
        if self.kind == AuxiliaryRouteKind::LegacyCompat {
            return false;
        }
        if self.route.authority() == RouteAuthority::HostFallback {
            return false;
        }
        if self.route.credential_route() == RouteCredentialRoute::None {
            return false;
        }
        let id = self.route.instance_id();
        if id.is_empty() || id == AUX_LEGACY_COMPAT_INSTANCE {
            return false;
        }
        true
    }

    /// Bind exact-route 401 attribution for this aux sample.
    ///
    /// Replaces any session-primary attribution callback so a 401 cannot
    /// attribute to a sibling account. Truly non-authoritative
    /// (`LegacyCompat` / HostFallback / None-credential) routes clear the
    /// callback. Unverified BYOK pins with a known instance keep a
    /// route-bound callback for exact-instance telemetry.
    pub fn bind_attribution(
        &mut self,
        auth_manager: Option<&std::sync::Arc<crate::auth::AuthManager>>,
        session_id: Option<String>,
    ) {
        if !self.supports_route_bound_attribution() {
            self.inference.attribution_callback = None;
            return;
        }
        let Some(am) = auth_manager else {
            self.inference.attribution_callback = None;
            return;
        };
        let cell = std::sync::Arc::new(parking_lot::RwLock::new(Some(self.route.clone())));
        self.inference.attribution_callback =
            Some(crate::auth::attribution::ShellAttribution::new_with_route(
                am.clone(),
                session_id,
                cell,
            ));
    }

    /// Build a sampling client that retains the exact resolved route.
    pub fn client(&self) -> Result<xai_grok_inference::InferenceClient, String> {
        xai_grok_inference::InferenceClient::new_with_route_context(
            self.inference.clone(),
            Some(self.route.clone()),
        )
        .map_err(|e| format!("client construction failed: {e}"))
    }

    /// Secret-free purpose/kind snapshot for bounded metadata.
    pub fn disclosure_meta(&self) -> serde_json::Value {
        serde_json::json!({
            "purpose": self.purpose.as_str(),
            "kind": match self.kind {
                AuxiliaryRouteKind::SessionInherit => "session_inherit",
                AuxiliaryRouteKind::Explicit => "explicit",
                AuxiliaryRouteKind::LegacyCompat => "legacy_compat",
            },
            "instance_id": self.route.instance_id(),
            "authority": self.route.authority().as_str(),
            "operation": self.route.operation_partition(),
        })
    }
}

/// Build a tool-side web-search 401 callback for a resolved aux route.
///
/// Returns `None` for HostFallback / legacy / none-credential routes so
/// registration cannot inherit a sibling session tool callback. Unverified
/// BYOK pins with a known instance id receive a route-bound callback.
pub(crate) fn web_search_tool_attribution_for_route(
    auth_manager: &std::sync::Arc<crate::auth::AuthManager>,
    session_id: Option<String>,
    route: &ProviderRouteContext,
    kind: AuxiliaryRouteKind,
) -> Option<xai_grok_tools::SharedAttributionCallback> {
    let probe = ResolvedAuxiliaryRoute {
        purpose: AuxiliaryPurpose::WebSearch,
        kind,
        canonical_selection_id: None,
        upstream_model_id: String::new(),
        inference: InferenceConfig::default(),
        route: route.clone(),
    };
    if !probe.supports_route_bound_attribution() {
        return None;
    }
    Some(
        crate::auth::attribution::ShellAttribution::new_tool_callback_with_route(
            auth_manager.clone(),
            session_id,
            route.clone(),
        ),
    )
}

/// Inputs for one auxiliary route resolution.
#[derive(Clone, Copy)]
pub struct AuxiliaryRouteInputs<'a> {
    pub purpose: AuxiliaryPurpose,
    /// Configured selection: `@session` or a canonical/historical id.
    pub requested: &'a str,
    pub models_manager: &'a ModelsManager,
    /// Frozen session route; required for `@session` inheritance.
    pub frozen_session_route: Option<&'a ProviderRouteContext>,
    pub frozen_session_inference: &'a InferenceConfig,
    pub frozen_session_selection_id: &'a str,
    pub grok_home: Option<&'a Path>,
    pub session_key: Option<&'a str>,
    pub disable_api_key_auth: bool,
    pub alpha_test_key: Option<&'a str>,
    pub client_version: Option<&'a str>,
    pub client_identifier: Option<&'a str>,
    pub max_retries: Option<u32>,
    /// When true, allow catalog fallback across accounts for the same
    /// upstream wire id. Default is false (fail closed on sibling).
    pub allow_cross_account_fallback: bool,
    /// When true, an explicit pin that fails must not soft-fallback to session.
    pub explicit_pin_fail_closed: bool,
}

/// Resolve an exact auxiliary route handle.
pub fn resolve_auxiliary_route(
    inputs: AuxiliaryRouteInputs<'_>,
) -> Result<ResolvedAuxiliaryRoute, AuxiliaryRouteError> {
    let requested = inputs.requested.trim();
    if requested.is_empty() || requested == SESSION_ROUTE_SENTINEL {
        return resolve_session_inherit(inputs);
    }
    resolve_explicit_or_compat(inputs, requested)
}

fn resolve_session_inherit(
    inputs: AuxiliaryRouteInputs<'_>,
) -> Result<ResolvedAuxiliaryRoute, AuxiliaryRouteError> {
    let route = inputs
        .frozen_session_route
        .ok_or(AuxiliaryRouteError::SessionRouteRequired)?
        .with_operation_partition(inputs.purpose.operation_partition());
    let mut inference = inputs.frozen_session_inference.clone();
    stamp_session_local_sampler_fields(
        &mut inference,
        inputs.frozen_session_inference,
        inputs.client_identifier.map(str::to_owned),
        inputs.max_retries,
    );
    let upstream = inference.model.clone();
    Ok(ResolvedAuxiliaryRoute {
        purpose: inputs.purpose,
        kind: AuxiliaryRouteKind::SessionInherit,
        canonical_selection_id: Some(inputs.frozen_session_selection_id.to_owned()),
        upstream_model_id: upstream,
        inference,
        route,
    })
}

fn resolve_explicit_or_compat(
    inputs: AuxiliaryRouteInputs<'_>,
    requested: &str,
) -> Result<ResolvedAuxiliaryRoute, AuxiliaryRouteError> {
    let models = inputs.models_manager.models();
    let origins = inputs.models_manager.catalog_origins();
    let identity = resolve_model_identity_with_origins(&models, &origins, requested);

    match identity {
        ModelIdentityResolution::Ambiguous { input, .. } => {
            Err(AuxiliaryRouteError::Ambiguous { input })
        }
        ModelIdentityResolution::Resolved(resolved) => {
            let selection = resolved.canonical_id.as_str().to_owned();
            // Cross-account: reject when the resolved key is a generated
            // additional-account entry and the input was an unscoped alias
            // unless fallback is explicitly enabled.
            if !inputs.allow_cross_account_fallback
                && crate::agent::model_identity::is_additional_account_key(&selection, &origins)
                && split_first_colon(requested).is_none()
            {
                return Err(AuxiliaryRouteError::Ambiguous {
                    input: requested.to_owned(),
                });
            }
            let entry = models.get(&selection).ok_or_else(|| {
                if inputs.explicit_pin_fail_closed {
                    AuxiliaryRouteError::ExplicitPinFailed {
                        selection: selection.clone(),
                    }
                } else {
                    AuxiliaryRouteError::Missing {
                        input: requested.to_owned(),
                    }
                }
            })?;
            build_explicit_route(inputs, &selection, entry)
        }
        ModelIdentityResolution::Missing { input } => {
            // Catalog keys that look namespaced never fall into legacy_compat.
            if split_first_colon(&input).is_some() {
                return Err(AuxiliaryRouteError::NamespacedHijackRejected { input });
            }
            // Web search: hidden default only for the exact historical default.
            if inputs.purpose == AuxiliaryPurpose::WebSearch {
                if input == crate::models::default_web_search_model() {
                    return build_legacy_compat(inputs, &input);
                }
                return Err(if inputs.explicit_pin_fail_closed {
                    AuxiliaryRouteError::ExplicitPinFailed { selection: input }
                } else {
                    AuxiliaryRouteError::Missing { input }
                });
            }
            if is_historical_first_party_wire_slug(&input) {
                return build_legacy_compat(inputs, &input);
            }
            Err(if inputs.explicit_pin_fail_closed {
                AuxiliaryRouteError::ExplicitPinFailed { selection: input }
            } else {
                AuxiliaryRouteError::Missing { input }
            })
        }
    }
}

fn build_explicit_route(
    inputs: AuxiliaryRouteInputs<'_>,
    selection: &str,
    entry: &ModelEntry,
) -> Result<ResolvedAuxiliaryRoute, AuxiliaryRouteError> {
    let credentials = crate::agent::config::resolve_credentials_enforced(
        entry,
        inputs.session_key,
        inputs.disable_api_key_auth,
    );
    // Mirror resolve_aux_model_inference_config / resolve_web_search: a catalog
    // entry without a usable key (and auth-provider cold cache) fails closed
    // rather than borrowing a sibling credential or inventing a host route.
    if credentials.api_key.is_none() {
        return Err(AuxiliaryRouteError::CredentialUnavailable {
            selection: selection.to_owned(),
        });
    }
    let mut inference = try_inference_config_for_model(
        entry,
        credentials,
        inputs.alpha_test_key.map(str::to_owned),
        inputs.client_version.map(str::to_owned),
        None,
        None,
    )
    .map_err(|e| AuxiliaryRouteError::ConstructionFailed {
        selection: selection.to_owned(),
        detail: e.to_string(),
    })?;
    stamp_session_local_sampler_fields(
        &mut inference,
        inputs.frozen_session_inference,
        inputs.client_identifier.map(str::to_owned),
        inputs.max_retries,
    );
    let upstream = upstream_id_for_entry(entry)
        .map(|u| u.into_string())
        .unwrap_or_else(|_| inference.model.clone());
    // Ensure wire model is the upstream id only.
    inference.model = upstream.clone();

    let route = crate::session::route_context::resolve_for_models_manager_with_selection(
        &inference,
        inputs.models_manager,
        selection,
        inputs.grok_home,
    )
    .with_operation_partition(inputs.purpose.operation_partition());

    Ok(ResolvedAuxiliaryRoute {
        purpose: inputs.purpose,
        kind: AuxiliaryRouteKind::Explicit,
        canonical_selection_id: Some(selection.to_owned()),
        upstream_model_id: upstream,
        inference,
        route,
    })
}

fn build_legacy_compat(
    inputs: AuxiliaryRouteInputs<'_>,
    wire_slug: &str,
) -> Result<ResolvedAuxiliaryRoute, AuxiliaryRouteError> {
    let endpoints = inputs.models_manager.endpoints();
    let bearer = inputs
        .session_key
        .map(str::to_owned)
        .or_else(|| crate::agent::auth_method::read_xai_api_key_env().ok())
        .or_else(|| endpoints.deployment_key.clone())
        .ok_or_else(|| AuxiliaryRouteError::CredentialUnavailable {
            selection: wire_slug.to_owned(),
        })?;

    let upstream =
        UpstreamModelId::new(wire_slug).map_err(|e| AuxiliaryRouteError::ConstructionFailed {
            selection: wire_slug.to_owned(),
            detail: e.to_string(),
        })?;

    let entry = ModelEntry {
        info: crate::agent::config::ModelInfo {
            user_selectable: false,
            id: None,
            model: upstream.as_str().to_owned(),
            base_url: endpoints.resolve_inference_base_url(),
            name: None,
            description: None,
            max_completion_tokens: None,
            temperature: None,
            top_p: None,
            api_backend: xai_grok_inference::ApiBackend::Responses,
            auth_scheme: Default::default(),
            extra_headers: indexmap::IndexMap::new(),
            context_window: std::num::NonZeroU64::new(200_000).unwrap(),
            auto_compact_threshold_percent: None,
            system_prompt_label: None,
            use_concise: false,
            agent_type: crate::agent::config::default_agent_type(),
            inference_idle_timeout_secs: None,
            max_retries: None,
            hidden: true,
            supported_in_api: true,
            reasoning_effort: None,
            supports_reasoning_effort: None,
            reasoning_efforts: Vec::new(),
            reasoning_effort_selection: xai_grok_inference_types::ReasoningEffortSelection::Unknown,
            supports_backend_search: false,
            compactions_remaining: None,
            compaction_at_tokens: None,
            show_model_fingerprint: false,
            stream_tool_calls: None,
            laziness_detector: crate::agent::config::LazinessDetectorPerModelConfig::default(),
            supports_tools: None,
            supports_native_schema: None,
            supports_strict_tools: None,
            supports_image_input: None,
            supports_audio_input: None,
            supports_video_input: None,
            execution_backend: crate::agent::execution_backend::ExecutionBackend::NativeInference,
        },
        model_provider: None,
        api_key: Some(bearer),
        env_key: None,
        auth_provider: None,
        api_base_url: None,
    };
    let credentials = crate::agent::config::resolve_credentials_enforced(
        &entry,
        inputs.session_key,
        inputs.disable_api_key_auth,
    );
    let mut inference = try_inference_config_for_model(
        &entry,
        credentials,
        inputs.alpha_test_key.map(str::to_owned),
        inputs.client_version.map(str::to_owned),
        None,
        None,
    )
    .map_err(|e| AuxiliaryRouteError::ConstructionFailed {
        selection: wire_slug.to_owned(),
        detail: e.to_string(),
    })?;
    inference.model = upstream.as_str().to_owned();
    stamp_session_local_sampler_fields(
        &mut inference,
        inputs.frozen_session_inference,
        inputs.client_identifier.map(str::to_owned),
        inputs.max_retries,
    );

    // Non-authoritative sidecar: never claims built-in xAI session credential
    // and never matches exact-account 401 repair.
    let route = ProviderRouteContext::builder()
        .instance_id(AUX_LEGACY_COMPAT_INSTANCE)
        .provider_kind(RouteProviderKind::Xai)
        .api_surface(RouteApiSurface::OpenAiCompatibleSubset)
        .credential_route(RouteCredentialRoute::None)
        .registry_generation(0)
        .binding_generation(0)
        .credential_generation(0)
        .authority(RouteAuthority::HostFallback)
        .origin_from_base_url(&inference.base_url)
        .model_partition(upstream.as_str())
        .operation_partition(inputs.purpose.operation_partition())
        .build()
        .map_err(|e| AuxiliaryRouteError::ConstructionFailed {
            selection: wire_slug.to_owned(),
            detail: e,
        })?;

    Ok(ResolvedAuxiliaryRoute {
        purpose: inputs.purpose,
        kind: AuxiliaryRouteKind::LegacyCompat,
        canonical_selection_id: None,
        upstream_model_id: upstream.into_string(),
        inference,
        route,
    })
}

/// Validated historical first-party wire slug: non-namespaced, `grok-…` shape,
/// or an exact compiled-in default model id.
fn is_historical_first_party_wire_slug(id: &str) -> bool {
    if id.is_empty() || id.len() > xai_grok_models::MAX_MODEL_ID_LEN {
        return false;
    }
    if split_first_colon(id).is_some() {
        return false;
    }
    if id == crate::models::default_model()
        || id == crate::models::default_web_search_model()
        || id == crate::models::default_session_summary_model()
        || id == crate::models::default_image_description_model()
    {
        return true;
    }
    // Historical first-party wire slugs are `grok-` + safe graphic chars.
    if let Some(rest) = id.strip_prefix("grok-") {
        return !rest.is_empty()
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'));
    }
    false
}

/// Production seam: resolve media describe from an explicit pin or `@session`.
///
/// Explicit pins fail closed (no silent session fallback). `@session` requires
/// a frozen route. `purpose` must be a media purpose (`MediaDescribe`,
/// `MediaVideo`, or `MediaPdf`).
pub fn resolve_media_describe_route(
    mut inputs: AuxiliaryRouteInputs<'_>,
    purpose: AuxiliaryPurpose,
) -> Result<ResolvedAuxiliaryRoute, AuxiliaryRouteError> {
    inputs.purpose = purpose;
    inputs.explicit_pin_fail_closed = true;
    resolve_auxiliary_route(inputs)
}

/// Fixed xAI streaming-STT transport labels accepted as `MediaStt` pins.
pub const MEDIA_STT_ROUTE_ALIASES: &[&str] = &[
    SESSION_ROUTE_SENTINEL,
    "xai",
    "xai-stt",
    "xai-streaming-stt",
];

/// Production seam: resolve file-audio STT under the central aux contract.
///
/// Transport remains the typed xAI streaming STT WebSocket, but Gate B still
/// requires an exact `MediaStt` route handle first:
/// - accepted pins: empty/`None`/`@session`/`xai`/`xai-stt`/`xai-streaming-stt`
/// - inherits the **frozen session** route with `media_stt` operation partition
/// - fails closed when the frozen route is not first-party xAI (never silently
///   borrow a sibling/current AuthManager xAI bearer while the session pin is
///   a foreign account)
/// - fails closed for any other catalog/audio-model slug
pub fn resolve_media_stt_route(
    mut inputs: AuxiliaryRouteInputs<'_>,
    audio_model_pin: Option<&str>,
) -> Result<ResolvedAuxiliaryRoute, AuxiliaryRouteError> {
    inputs.purpose = AuxiliaryPurpose::MediaStt;
    inputs.explicit_pin_fail_closed = true;
    let pin = audio_model_pin.map(str::trim).filter(|s| !s.is_empty());
    match pin {
        None => {}
        Some(p) if MEDIA_STT_ROUTE_ALIASES.iter().any(|a| *a == p) => {}
        Some(other) => {
            return Err(AuxiliaryRouteError::ExplicitPinFailed {
                selection: other.to_owned(),
            });
        }
    }
    let frozen = inputs
        .frozen_session_route
        .ok_or(AuxiliaryRouteError::SessionRouteRequired)?;
    // STT transport is xAI-only. Refuse non-xAI session pins so production
    // never uses a current/sibling xAI bearer under a foreign session route.
    let is_xai = matches!(
        frozen.provider_kind(),
        xai_grok_inference::RouteProviderKind::Xai
    ) || matches!(frozen.credential_route(), RouteCredentialRoute::XaiSession);
    if !is_xai {
        return Err(AuxiliaryRouteError::ConstructionFailed {
            selection: pin.unwrap_or(SESSION_ROUTE_SENTINEL).to_owned(),
            detail: "xAI streaming STT requires an exact xAI session route".to_owned(),
        });
    }
    // Always inherit frozen session with MediaStt purpose (exact generations).
    inputs.requested = SESSION_ROUTE_SENTINEL;
    resolve_auxiliary_route(inputs)
}

/// Soft-fallback title path: keep primary upstream wire model **and** exact
/// primary route with `SessionTitle` operation partition (never drop route).
pub fn session_title_soft_fallback_route(
    frozen_session_route: &ProviderRouteContext,
    frozen_session_inference: &InferenceConfig,
    frozen_session_selection_id: &str,
) -> ResolvedAuxiliaryRoute {
    let route = frozen_session_route
        .with_operation_partition(AuxiliaryPurpose::SessionTitle.operation_partition());
    let mut inference = frozen_session_inference.clone();
    // Wire model stays the true primary upstream — never an unresolved slug.
    ResolvedAuxiliaryRoute {
        purpose: AuxiliaryPurpose::SessionTitle,
        kind: AuxiliaryRouteKind::SessionInherit,
        canonical_selection_id: Some(frozen_session_selection_id.to_owned()),
        upstream_model_id: inference.model.clone(),
        inference,
        route,
    }
}

/// Paired title-sampler construction that cannot drop route provenance.
#[derive(Clone, Debug)]
pub struct SessionTitleSamplerPairing {
    pub route: ResolvedAuxiliaryRoute,
}

impl SessionTitleSamplerPairing {
    /// Resolve the session-title sampler for a new session boundary.
    pub fn for_session_new(inputs: AuxiliaryRouteInputs<'_>) -> Result<Self, AuxiliaryRouteError> {
        let mut inputs = inputs;
        inputs.purpose = AuxiliaryPurpose::SessionTitle;
        let route = resolve_auxiliary_route(inputs)?;
        Ok(Self { route })
    }

    pub fn model(&self) -> &str {
        &self.route.upstream_model_id
    }

    pub fn client(&self) -> Result<xai_grok_inference::InferenceClient, String> {
        self.route.client()
    }
}

/// Apply compaction-specific sanitization to a resolved route's inference
/// config (clear OpenRouter hidden failover, reasoning, backend search).
pub fn sanitize_compaction_inference(cfg: &mut InferenceConfig) {
    cfg.openrouter_fallback_models.clear();
    cfg.openrouter_provider_preferences = None;
    cfg.openrouter_plugins.clear();
    cfg.openrouter_pacing = false;
    cfg.reasoning_effort = None;
    cfg.supports_backend_search = false;
    cfg.compactions_remaining = None;
    cfg.compaction_at_tokens = None;
    cfg.doom_loop_recovery = None;
}

/// Convenience builder for session-actor call sites.
pub fn aux_inputs_from_session<'a>(
    purpose: AuxiliaryPurpose,
    requested: &'a str,
    models_manager: &'a ModelsManager,
    frozen_session_route: Option<&'a ProviderRouteContext>,
    frozen_session_inference: &'a InferenceConfig,
    frozen_session_selection_id: &'a str,
    grok_home: Option<&'a Path>,
    session_key: Option<&'a str>,
    disable_api_key_auth: bool,
    alpha_test_key: Option<&'a str>,
    client_version: Option<&'a str>,
    client_identifier: Option<&'a str>,
    max_retries: Option<u32>,
) -> AuxiliaryRouteInputs<'a> {
    AuxiliaryRouteInputs {
        purpose,
        requested,
        models_manager,
        frozen_session_route,
        frozen_session_inference,
        frozen_session_selection_id,
        grok_home,
        session_key,
        disable_api_key_auth,
        alpha_test_key,
        client_version,
        client_identifier,
        max_retries,
        allow_cross_account_fallback: false,
        explicit_pin_fail_closed: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::ModelEntry;
    use crate::agent::model_providers::{
        ModelProviderConfig, ModelProviderKind, ResolvedModelProvider,
    };
    use agent_client_protocol as acp;
    use indexmap::IndexMap;
    use tempfile::tempdir;

    fn test_entry(wire: &str, provider_id: &str, base_url: &str, key: Option<&str>) -> ModelEntry {
        ModelEntry {
            info: crate::agent::config::ModelInfo {
                user_selectable: true,
                id: None,
                model: wire.to_owned(),
                base_url: base_url.to_owned(),
                name: None,
                description: None,
                max_completion_tokens: None,
                temperature: None,
                top_p: None,
                api_backend: xai_grok_inference::ApiBackend::Responses,
                auth_scheme: Default::default(),
                extra_headers: IndexMap::new(),
                context_window: std::num::NonZeroU64::new(200_000).unwrap(),
                auto_compact_threshold_percent: None,
                system_prompt_label: None,
                use_concise: false,
                agent_type: crate::agent::config::default_agent_type(),
                inference_idle_timeout_secs: None,
                max_retries: None,
                hidden: false,
                supported_in_api: true,
                reasoning_effort: None,
                supports_reasoning_effort: None,
                reasoning_efforts: Vec::new(),
                reasoning_effort_selection:
                    xai_grok_inference_types::ReasoningEffortSelection::Unknown,
                supports_backend_search: false,
                compactions_remaining: None,
                compaction_at_tokens: None,
                show_model_fingerprint: false,
                stream_tool_calls: None,
                laziness_detector: crate::agent::config::LazinessDetectorPerModelConfig::default(),
                supports_tools: None,
                supports_native_schema: None,
                supports_strict_tools: None,
                supports_image_input: None,
                supports_audio_input: None,
                supports_video_input: None,
                execution_backend:
                    crate::agent::execution_backend::ExecutionBackend::NativeInference,
            },
            model_provider: Some(ResolvedModelProvider {
                id: provider_id.to_owned(),
                kind: ModelProviderKind::OpenAiCompatible,
                openrouter_fallback_models: vec![],
                openrouter_provider_preferences: None,
                openrouter_plugins: vec![],
                openrouter_pacing: false,
                command: vec![],
            }),
            api_key: key.map(str::to_owned),
            env_key: None,
            auth_provider: None,
            api_base_url: None,
        }
    }

    fn manager_with(
        models: IndexMap<String, ModelEntry>,
        providers: IndexMap<String, ModelProviderConfig>,
        current: &str,
        home: &Path,
    ) -> ModelsManager {
        let mut config = crate::agent::config::Config::default();
        config.model_providers = providers;
        ModelsManager::new(
            None,
            models,
            acp::ModelId::new(current),
            std::sync::Arc::new(crate::auth::AuthManager::new(
                home,
                crate::auth::GrokComConfig::default(),
            )),
            config,
        )
    }

    fn session_inference() -> InferenceConfig {
        InferenceConfig {
            api_key: Some("session-key".into()),
            model: "session-wire".into(),
            base_url: "https://account-a.example/v1".into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::Custom,
            ..InferenceConfig::default()
        }
    }

    fn frozen_route() -> ProviderRouteContext {
        ProviderRouteContext::builder()
            .instance_id("account-a")
            .provider_kind(RouteProviderKind::OpenAiCompatible)
            .api_surface(RouteApiSurface::OpenAiCompatibleSubset)
            .credential_route(RouteCredentialRoute::ApiKey)
            .binding_generation(1)
            .authority(RouteAuthority::Authoritative)
            .origin_from_base_url("https://account-a.example/v1")
            .model_partition("session-wire")
            .build()
            .unwrap()
    }

    #[test]
    fn session_inherit_copies_frozen_route_with_operation() {
        let dir = tempdir().unwrap();
        let mut models = IndexMap::new();
        models.insert(
            "session-a".into(),
            test_entry(
                "session-wire",
                "account-a",
                "https://account-a.example/v1",
                Some("key-a"),
            ),
        );
        let mgr = manager_with(models, IndexMap::new(), "session-a", dir.path());
        let frozen = frozen_route();
        let session = session_inference();
        let resolved = resolve_auxiliary_route(AuxiliaryRouteInputs {
            purpose: AuxiliaryPurpose::Compaction,
            requested: "@session",
            models_manager: &mgr,
            frozen_session_route: Some(&frozen),
            frozen_session_inference: &session,
            frozen_session_selection_id: "session-a",
            grok_home: Some(dir.path()),
            session_key: Some("session-jwt"),
            disable_api_key_auth: false,
            alpha_test_key: None,
            client_version: None,
            client_identifier: None,
            max_retries: Some(2),
            allow_cross_account_fallback: false,
            explicit_pin_fail_closed: false,
        })
        .unwrap();
        assert_eq!(resolved.kind, AuxiliaryRouteKind::SessionInherit);
        assert_eq!(resolved.route.instance_id(), "account-a");
        assert_eq!(resolved.route.operation_partition(), "compaction");
        assert_eq!(resolved.upstream_model_id, "session-wire");
        assert_eq!(resolved.route.binding_generation(), 1);
    }

    #[test]
    fn session_inherit_without_frozen_route_fails_closed() {
        let dir = tempdir().unwrap();
        let mgr = manager_with(IndexMap::new(), IndexMap::new(), "default", dir.path());
        let session = session_inference();
        let err = resolve_auxiliary_route(AuxiliaryRouteInputs {
            purpose: AuxiliaryPurpose::MediaDescribe,
            requested: "@session",
            models_manager: &mgr,
            frozen_session_route: None,
            frozen_session_inference: &session,
            frozen_session_selection_id: "default",
            grok_home: Some(dir.path()),
            session_key: None,
            disable_api_key_auth: false,
            alpha_test_key: None,
            client_version: None,
            client_identifier: None,
            max_retries: None,
            allow_cross_account_fallback: false,
            explicit_pin_fail_closed: true,
        })
        .unwrap_err();
        assert!(matches!(err, AuxiliaryRouteError::SessionRouteRequired));
    }

    #[test]
    fn sibling_accounts_do_not_share_credentials() {
        let dir = tempdir().unwrap();
        let mut models = IndexMap::new();
        models.insert(
            "account-a:gpt-4o".into(),
            test_entry(
                "gpt-4o",
                "account-a",
                "https://account-a.example/v1",
                Some("key-a"),
            ),
        );
        models.insert(
            "account-b:gpt-4o".into(),
            test_entry(
                "gpt-4o",
                "account-b",
                "https://account-b.example/v1",
                Some("key-b"),
            ),
        );
        let mut providers = IndexMap::new();
        providers.insert(
            "account-a".into(),
            ModelProviderConfig {
                base_url: Some("https://account-a.example/v1".into()),
                ..Default::default()
            },
        );
        providers.insert(
            "account-b".into(),
            ModelProviderConfig {
                base_url: Some("https://account-b.example/v1".into()),
                ..Default::default()
            },
        );
        let mgr = manager_with(models, providers, "account-a:gpt-4o", dir.path());
        let frozen = frozen_route();
        let session = session_inference();
        let a = resolve_auxiliary_route(AuxiliaryRouteInputs {
            purpose: AuxiliaryPurpose::Compaction,
            requested: "account-a:gpt-4o",
            models_manager: &mgr,
            frozen_session_route: Some(&frozen),
            frozen_session_inference: &session,
            frozen_session_selection_id: "account-a:gpt-4o",
            grok_home: Some(dir.path()),
            session_key: None,
            disable_api_key_auth: false,
            alpha_test_key: None,
            client_version: None,
            client_identifier: None,
            max_retries: None,
            allow_cross_account_fallback: false,
            explicit_pin_fail_closed: true,
        })
        .unwrap();
        let b = resolve_auxiliary_route(AuxiliaryRouteInputs {
            purpose: AuxiliaryPurpose::Compaction,
            requested: "account-b:gpt-4o",
            models_manager: &mgr,
            frozen_session_route: Some(&frozen),
            frozen_session_inference: &session,
            frozen_session_selection_id: "account-a:gpt-4o",
            grok_home: Some(dir.path()),
            session_key: None,
            disable_api_key_auth: false,
            alpha_test_key: None,
            client_version: None,
            client_identifier: None,
            max_retries: None,
            allow_cross_account_fallback: false,
            explicit_pin_fail_closed: true,
        })
        .unwrap();
        assert_eq!(a.route.instance_id(), "account-a");
        assert_eq!(b.route.instance_id(), "account-b");
        assert_eq!(a.inference.api_key.as_deref(), Some("key-a"));
        assert_eq!(b.inference.api_key.as_deref(), Some("key-b"));
        assert_ne!(a.route.instance_id(), b.route.instance_id());
    }

    #[test]
    fn namespaced_canonical_hijack_is_rejected() {
        let dir = tempdir().unwrap();
        let mgr = manager_with(IndexMap::new(), IndexMap::new(), "default", dir.path());
        let frozen = frozen_route();
        let session = session_inference();
        let err = resolve_auxiliary_route(AuxiliaryRouteInputs {
            purpose: AuxiliaryPurpose::WebSearch,
            requested: "openai_work:gpt-4o",
            models_manager: &mgr,
            frozen_session_route: Some(&frozen),
            frozen_session_inference: &session,
            frozen_session_selection_id: "default",
            grok_home: Some(dir.path()),
            session_key: Some("session-jwt"),
            disable_api_key_auth: false,
            alpha_test_key: None,
            client_version: None,
            client_identifier: None,
            max_retries: None,
            allow_cross_account_fallback: false,
            explicit_pin_fail_closed: true,
        })
        .unwrap_err();
        assert!(matches!(
            err,
            AuxiliaryRouteError::NamespacedHijackRejected { .. }
        ));
    }

    #[test]
    fn legacy_compat_is_non_authoritative() {
        let dir = tempdir().unwrap();
        let mgr = manager_with(IndexMap::new(), IndexMap::new(), "default", dir.path());
        let frozen = frozen_route();
        let session = session_inference();
        let resolved = resolve_auxiliary_route(AuxiliaryRouteInputs {
            purpose: AuxiliaryPurpose::SessionTitle,
            requested: "grok-legacy-test",
            models_manager: &mgr,
            frozen_session_route: Some(&frozen),
            frozen_session_inference: &session,
            frozen_session_selection_id: "default",
            grok_home: Some(dir.path()),
            session_key: Some("session-jwt"),
            disable_api_key_auth: false,
            alpha_test_key: None,
            client_version: None,
            client_identifier: None,
            max_retries: None,
            allow_cross_account_fallback: false,
            explicit_pin_fail_closed: false,
        })
        .unwrap();
        assert_eq!(resolved.kind, AuxiliaryRouteKind::LegacyCompat);
        assert_eq!(resolved.route.instance_id(), AUX_LEGACY_COMPAT_INSTANCE);
        assert_eq!(resolved.route.authority(), RouteAuthority::HostFallback);
        assert_eq!(
            resolved.route.credential_route(),
            RouteCredentialRoute::None
        );
        assert_eq!(resolved.route.binding_generation(), 0);
        assert_eq!(resolved.route.registry_generation(), 0);
        assert_eq!(resolved.upstream_model_id, "grok-legacy-test");
        assert!(resolved.canonical_selection_id.is_none());
    }

    #[test]
    fn legacy_compat_rejects_failed_catalog_key() {
        // A catalog key present but without credentials is not legacy_compat.
        let dir = tempdir().unwrap();
        let mut models = IndexMap::new();
        models.insert(
            "no-creds-model".into(),
            test_entry(
                "no-creds-model",
                "account-a",
                "https://account-a.example/v1",
                None,
            ),
        );
        let mgr = manager_with(models, IndexMap::new(), "no-creds-model", dir.path());
        let frozen = frozen_route();
        let session = session_inference();
        // Auth-provider free entry with no key + no session → credential fail
        // when auth scheme requires a key. For OpenAiCompatible with no key,
        // resolution may still produce a config; the important property is
        // kind != LegacyCompat for a catalog hit.
        let err = resolve_auxiliary_route(AuxiliaryRouteInputs {
            purpose: AuxiliaryPurpose::Compaction,
            requested: "no-creds-model",
            models_manager: &mgr,
            frozen_session_route: Some(&frozen),
            frozen_session_inference: &session,
            frozen_session_selection_id: "no-creds-model",
            grok_home: Some(dir.path()),
            session_key: None,
            disable_api_key_auth: false,
            alpha_test_key: None,
            client_version: None,
            client_identifier: None,
            max_retries: None,
            allow_cross_account_fallback: false,
            explicit_pin_fail_closed: true,
        })
        .expect_err("catalog hit without credentials must fail closed");
        match &err {
            AuxiliaryRouteError::CredentialUnavailable { selection }
                if selection == "no-creds-model" => {}
            other => panic!(
                "expected CredentialUnavailable for catalog key without credentials, got {other:?}"
            ),
        }
    }

    #[test]
    fn web_search_retains_resolved_route_no_origin_blind_fallback() {
        let dir = tempdir().unwrap();
        let mut models = IndexMap::new();
        models.insert(
            "enterprise-search".into(),
            test_entry(
                "enterprise-search",
                "enterprise",
                "https://enterprise.example/v1",
                Some("enterprise-key"),
            ),
        );
        let mut providers = IndexMap::new();
        providers.insert(
            "enterprise".into(),
            ModelProviderConfig {
                base_url: Some("https://enterprise.example/v1".into()),
                ..Default::default()
            },
        );
        let mgr = manager_with(models, providers, "enterprise-search", dir.path());
        let frozen = frozen_route();
        let session = session_inference();
        let resolved = resolve_auxiliary_route(AuxiliaryRouteInputs {
            purpose: AuxiliaryPurpose::WebSearch,
            requested: "enterprise-search",
            models_manager: &mgr,
            frozen_session_route: Some(&frozen),
            frozen_session_inference: &session,
            frozen_session_selection_id: "session-a",
            grok_home: Some(dir.path()),
            session_key: None,
            disable_api_key_auth: false,
            alpha_test_key: None,
            client_version: None,
            client_identifier: None,
            max_retries: None,
            allow_cross_account_fallback: false,
            explicit_pin_fail_closed: true,
        })
        .unwrap();
        assert_eq!(resolved.kind, AuxiliaryRouteKind::Explicit);
        assert_eq!(resolved.route.instance_id(), "enterprise");
        assert_eq!(resolved.upstream_model_id, "enterprise-search");
        assert_eq!(
            resolved.inference.api_key.as_deref(),
            Some("enterprise-key")
        );
        assert_eq!(resolved.route.operation_partition(), "web_search");
        // Missing non-default web-search pin does not invent an origin-blind path.
        let missing = resolve_auxiliary_route(AuxiliaryRouteInputs {
            purpose: AuxiliaryPurpose::WebSearch,
            requested: "not-a-real-search-model",
            models_manager: &mgr,
            frozen_session_route: Some(&frozen),
            frozen_session_inference: &session,
            frozen_session_selection_id: "session-a",
            grok_home: Some(dir.path()),
            session_key: Some("session-jwt"),
            disable_api_key_auth: false,
            alpha_test_key: None,
            client_version: None,
            client_identifier: None,
            max_retries: None,
            allow_cross_account_fallback: false,
            explicit_pin_fail_closed: true,
        });
        assert!(missing.is_err());
    }

    #[test]
    fn title_boundary_retains_route_via_pairing() {
        let dir = tempdir().unwrap();
        let mut models = IndexMap::new();
        models.insert(
            "title-model".into(),
            test_entry(
                "title-wire",
                "title-prov",
                "https://title.example/v1",
                Some("title-key"),
            ),
        );
        let mgr = manager_with(models, IndexMap::new(), "title-model", dir.path());
        let frozen = frozen_route();
        let session = session_inference();
        let pair = SessionTitleSamplerPairing::for_session_new(AuxiliaryRouteInputs {
            purpose: AuxiliaryPurpose::SessionTitle,
            requested: "title-model",
            models_manager: &mgr,
            frozen_session_route: Some(&frozen),
            frozen_session_inference: &session,
            frozen_session_selection_id: "session-a",
            grok_home: Some(dir.path()),
            session_key: None,
            disable_api_key_auth: false,
            alpha_test_key: None,
            client_version: None,
            client_identifier: Some("cli"),
            max_retries: Some(1),
            allow_cross_account_fallback: false,
            explicit_pin_fail_closed: false,
        })
        .unwrap();
        assert_eq!(pair.model(), "title-wire");
        assert_eq!(pair.route.kind, AuxiliaryRouteKind::Explicit);
        assert_eq!(pair.route.route.operation_partition(), "session_title");
        assert!(pair.route.route.instance_id() == "title-prov");
    }

    #[test]
    fn media_describe_explicit_pin_fails_closed() {
        let dir = tempdir().unwrap();
        let mgr = manager_with(IndexMap::new(), IndexMap::new(), "default", dir.path());
        let frozen = frozen_route();
        let session = session_inference();
        let err = resolve_media_describe_route(
            AuxiliaryRouteInputs {
                purpose: AuxiliaryPurpose::MediaDescribe,
                requested: "missing-vision-pin",
                models_manager: &mgr,
                frozen_session_route: Some(&frozen),
                frozen_session_inference: &session,
                frozen_session_selection_id: "default",
                grok_home: Some(dir.path()),
                session_key: Some("session-jwt"),
                disable_api_key_auth: false,
                alpha_test_key: None,
                client_version: None,
                client_identifier: None,
                max_retries: None,
                allow_cross_account_fallback: false,
                explicit_pin_fail_closed: true,
            },
            AuxiliaryPurpose::MediaDescribe,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            AuxiliaryRouteError::ExplicitPinFailed { .. } | AuxiliaryRouteError::Missing { .. }
        ));
    }

    #[test]
    fn disclosure_meta_is_secret_free() {
        let dir = tempdir().unwrap();
        let mgr = manager_with(IndexMap::new(), IndexMap::new(), "default", dir.path());
        let frozen = frozen_route();
        let session = session_inference();
        let resolved = resolve_auxiliary_route(AuxiliaryRouteInputs {
            purpose: AuxiliaryPurpose::PromptSuggest,
            requested: "@session",
            models_manager: &mgr,
            frozen_session_route: Some(&frozen),
            frozen_session_inference: &session,
            frozen_session_selection_id: "session-a",
            grok_home: Some(dir.path()),
            session_key: Some("super-secret-jwt-token"),
            disable_api_key_auth: false,
            alpha_test_key: Some("alpha-secret"),
            client_version: None,
            client_identifier: None,
            max_retries: None,
            allow_cross_account_fallback: false,
            explicit_pin_fail_closed: false,
        })
        .unwrap();
        let meta = resolved.disclosure_meta().to_string();
        assert!(!meta.contains("super-secret"));
        assert!(!meta.contains("alpha-secret"));
        assert!(!meta.contains("session-key"));
        assert!(meta.contains("prompt_suggest"));
        assert!(meta.contains("session_inherit"));
    }

    #[test]
    fn operation_partition_is_applied_to_explicit_routes() {
        let dir = tempdir().unwrap();
        let mut models = IndexMap::new();
        models.insert(
            "m".into(),
            test_entry("m-wire", "p", "https://p.example/v1", Some("k")),
        );
        let mgr = manager_with(models, IndexMap::new(), "m", dir.path());
        let frozen = frozen_route();
        let session = session_inference();
        let resolved = resolve_auxiliary_route(AuxiliaryRouteInputs {
            purpose: AuxiliaryPurpose::GoalEvaluator,
            requested: "m",
            models_manager: &mgr,
            frozen_session_route: Some(&frozen),
            frozen_session_inference: &session,
            frozen_session_selection_id: "m",
            grok_home: Some(dir.path()),
            session_key: None,
            disable_api_key_auth: false,
            alpha_test_key: None,
            client_version: None,
            client_identifier: None,
            max_retries: None,
            allow_cross_account_fallback: false,
            explicit_pin_fail_closed: true,
        })
        .unwrap();
        assert_eq!(resolved.route.operation_partition(), "goal_evaluator");
        let partition = resolved.route.pacing_partition("m-wire");
        assert_eq!(partition.5, "goal_evaluator");
    }
}
