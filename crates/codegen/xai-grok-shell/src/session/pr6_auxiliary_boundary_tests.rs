//! PR6 production-boundary tests for auxiliary route seams.
//!
//! These exercise the live client/request carriers (route-aware
//! `InferenceClient`, attribution bind, title soft-fallback model field,
//! media video/PDF operation partitions, compaction sanitization) rather
//! than only re-asserting the resolver unit tests.

use super::auxiliary_route::*;
use crate::agent::config::ModelEntry;
use crate::agent::model_providers::{
    ModelProviderConfig, ModelProviderKind, ResolvedModelProvider,
};
use crate::agent::models::ModelsManager;
use agent_client_protocol as acp;
use indexmap::IndexMap;
use std::sync::Arc;
use tempfile::tempdir;
use xai_grok_inference::{
    InferenceConfig, ProviderRouteContext, RouteApiSurface, RouteAuthority, RouteCredentialRoute,
    RouteProviderKind,
};

fn entry(wire: &str, provider: &str, base: &str, key: Option<&str>) -> ModelEntry {
    ModelEntry {
        info: crate::agent::config::ModelInfo {
            user_selectable: true,
            id: None,
            model: wire.to_owned(),
            base_url: base.to_owned(),
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
            supports_file_input: None,
            output_has_text: None,
            supports_zdr: None,
            max_output_ceiling: None,
            provider_display_name: None,
            execution_backend: crate::agent::execution_backend::ExecutionBackend::NativeInference,
        },
        model_provider: Some(ResolvedModelProvider {
            id: provider.to_owned(),
            kind: ModelProviderKind::OpenAiCompatible,
            openrouter_fallback_models: vec![],
            openrouter_provider_preferences: None,
            openrouter_plugins: vec![],
            openrouter_pacing: false,
            max_completion_tokens: None,
            command: vec![],
        }),
        api_key: key.map(str::to_owned),
        env_key: None,
        auth_provider: None,
        api_base_url: None,
    }
}

fn manager(
    models: IndexMap<String, ModelEntry>,
    providers: IndexMap<String, ModelProviderConfig>,
    current: &str,
    home: &std::path::Path,
) -> ModelsManager {
    let mut config = crate::agent::config::Config::default();
    config.model_providers = providers;
    ModelsManager::new(
        None,
        models,
        acp::ModelId::new(current),
        Arc::new(crate::auth::AuthManager::new(
            home,
            crate::auth::GrokComConfig::default(),
        )),
        config,
    )
}

/// Register a provider instance in the home's on-disk runtime registry
/// (`config.toml`). `entry()` attaches a `model_provider` descriptor to each
/// catalog row, and the production route guard fails closed on instances that
/// are not configured on disk. A provided `api_key` makes the OpenAI-compatible
/// instance an API-key credential route, matching how a real BYOK provider
/// declares its key.
fn register_provider(
    home: &std::path::Path,
    id: &str,
    kind: &str,
    base_url: &str,
    api_key: Option<&str>,
) {
    use std::fmt::Write as _;
    let mut raw = std::fs::read_to_string(home.join("config.toml")).unwrap_or_default();
    let _ = writeln!(
        raw,
        "\n[model_providers.{id}]\nkind = \"{kind}\"\nbase_url = \"{base_url}\""
    );
    if let Some(key) = api_key {
        let _ = writeln!(raw, "api_key = \"{key}\"");
    }
    std::fs::write(home.join("config.toml"), raw).expect("write provider config");
    crate::provider_registry::runtime_cache::invalidate_for_home(home);
}

fn frozen() -> ProviderRouteContext {
    ProviderRouteContext::builder()
        .instance_id("account-a")
        .provider_kind(RouteProviderKind::OpenAiCompatible)
        .api_surface(RouteApiSurface::OpenAiCompatibleSubset)
        .credential_route(RouteCredentialRoute::ApiKey)
        .binding_generation(4)
        .authority(RouteAuthority::Authoritative)
        .origin_from_base_url("https://account-a.example/v1")
        .model_partition("session-wire")
        .build()
        .unwrap()
}

fn session_cfg() -> InferenceConfig {
    InferenceConfig {
        api_key: Some("session-key".into()),
        model: "session-wire".into(),
        base_url: "https://account-a.example/v1".into(),
        provider_identity: xai_grok_inference::config::ProviderIdentity::Custom,
        ..Default::default()
    }
}

fn base_inputs<'a>(
    purpose: AuxiliaryPurpose,
    requested: &'a str,
    mgr: &'a ModelsManager,
    frozen: &'a ProviderRouteContext,
    session: &'a InferenceConfig,
    home: &'a std::path::Path,
    session_key: Option<&'a str>,
) -> AuxiliaryRouteInputs<'a> {
    AuxiliaryRouteInputs {
        purpose,
        requested,
        models_manager: mgr,
        frozen_session_route: Some(frozen),
        frozen_session_inference: session,
        frozen_session_selection_id: "session-a",
        grok_home: Some(home),
        session_key,
        disable_api_key_auth: false,
        alpha_test_key: None,
        client_version: None,
        client_identifier: None,
        max_retries: None,
        allow_cross_account_fallback: false,
        explicit_pin_fail_closed: true,
    }
}

/// Production seam: compaction-style resolve → sanitize → route-aware client.
#[test]
fn compaction_prep_client_carries_exact_route_and_operation() {
    let dir = tempdir().unwrap();
    register_provider(
        dir.path(),
        "compact-p",
        "openai_compatible",
        "https://compact.example/v1",
        Some("compact-key"),
    );
    let mut models = IndexMap::new();
    models.insert(
        "compact".into(),
        entry(
            "compact-wire",
            "compact-p",
            "https://compact.example/v1",
            Some("compact-key"),
        ),
    );
    let mgr = manager(models, IndexMap::new(), "compact", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    let mut resolved = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::Compaction,
        "compact",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        None,
    ))
    .unwrap();
    sanitize_compaction_inference(&mut resolved.inference);
    let client = resolved.client().expect("compaction client");
    let route = client.route_context().expect("route retained on client");
    assert_eq!(route.instance_id(), "compact-p");
    assert_eq!(route.operation_partition(), "compaction");
    assert_eq!(resolved.upstream_model_id, "compact-wire");
    assert_eq!(client.model(), "compact-wire");
}

/// Compaction inherits the request max from configuration, exactly like the
/// main turn: catalog ceiling first, then the OpenRouter API default. Nothing
/// on the compaction path invents or strips a request budget.
#[test]
fn compaction_route_inherits_resolved_request_max_tokens() {
    use crate::agent::model_providers::OPENROUTER_DEFAULT_MAX_COMPLETION_TOKENS;

    let openrouter_entry = |ceiling: Option<u32>| {
        let mut e = entry(
            "z-ai/glm-5.3-flash",
            "zdr",
            "https://openrouter.ai/api/v1",
            Some("or-key"),
        );
        e.info.max_output_ceiling = ceiling;
        if let Some(provider) = e.model_provider.as_mut() {
            provider.kind = ModelProviderKind::OpenRouter;
        }
        e
    };

    let resolve_max = |ceiling: Option<u32>, home: &std::path::Path| {
        let mut models = IndexMap::new();
        models.insert("glm".into(), openrouter_entry(ceiling));
        let mgr = manager(models, IndexMap::new(), "glm", home);
        let session = session_cfg();
        let mut resolved = resolve_auxiliary_route(base_inputs(
            AuxiliaryPurpose::Compaction,
            "glm",
            &mgr,
            &frozen(),
            &session,
            home,
            None,
        ))
        .expect("compaction route resolves");
        sanitize_compaction_inference(&mut resolved.inference);
        resolved.inference
    };

    // A catalog ceiling fills the request budget, and survives sanitization.
    let dir = tempdir().unwrap();
    register_provider(
        dir.path(),
        "zdr",
        "openrouter",
        "https://openrouter.ai/api/v1",
        Some("or-key"),
    );
    let with_ceiling = resolve_max(Some(131_072), dir.path());
    assert_eq!(
        with_ceiling.max_completion_tokens,
        Some(131_072),
        "compaction must inherit the catalog ceiling, not drop it"
    );
    assert_eq!(with_ceiling.max_output_ceiling, Some(131_072));

    // No ceiling anywhere: the shared OpenRouter API default applies.
    let without_ceiling = resolve_max(None, dir.path());
    assert_eq!(
        without_ceiling.max_completion_tokens,
        Some(OPENROUTER_DEFAULT_MAX_COMPLETION_TOKENS),
        "compaction must fall back to the same API default as the main turn"
    );
}

/// Production seam: web-search resolve binds exact-route attribution (not sibling).
#[test]
fn web_search_prep_binds_exact_route_attribution_not_sibling() {
    let dir = tempdir().unwrap();
    register_provider(
        dir.path(),
        "ws-prov",
        "openai_compatible",
        "https://ws.example/v1",
        Some("ws-key"),
    );
    let mut models = IndexMap::new();
    models.insert(
        "ws".into(),
        entry(
            "ws-wire",
            "ws-prov",
            "https://ws.example/v1",
            Some("ws-key"),
        ),
    );
    let mgr = manager(models, IndexMap::new(), "ws", dir.path());
    let frozen = frozen();
    // Session primary carries a distinct sibling attribution callback marker.
    let mut session = session_cfg();
    session.attribution_callback = Some(crate::auth::attribution::ShellAttribution::new(
        Arc::new(crate::auth::AuthManager::new(
            dir.path(),
            crate::auth::GrokComConfig::default(),
        )),
        Some("session-sibling".into()),
    ));
    let mut resolved = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::WebSearch,
        "ws",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        None,
    ))
    .unwrap();
    // Stamp-from-session would have copied sibling callback; bind replaces or
    // clears it — never leaves a sibling primary identity on the aux pin.
    let am = Arc::new(crate::auth::AuthManager::new(
        dir.path(),
        crate::auth::GrokComConfig::default(),
    ));
    resolved.bind_attribution(Some(&am), Some("ws-session".into()));
    // Exact BYOK / configured pins (incl. Unverified) retain route-bound cb.
    assert!(
        resolved.supports_route_bound_attribution(),
        "ws-prov BYOK pin must support route-bound attribution (auth={:?})",
        resolved.route.authority()
    );
    assert!(
        resolved.inference.attribution_callback.is_some(),
        "exact web-search route must keep its own attribution after bind"
    );
    // Tool-side callback follows the same support gate.
    let tool_cb = web_search_tool_attribution_for_route(
        &am,
        Some("ws-session".into()),
        &resolved.route,
        resolved.kind,
    );
    assert!(
        tool_cb.is_some(),
        "web-search tool callback must be route-bound for exact BYOK"
    );
    // Registry seam: dedicated present; session primary must never be selected.
    let selected =
        xai_grok_tools::registry::types::select_web_search_attribution_callback(tool_cb.as_ref());
    assert!(selected.is_some());
    let absent = xai_grok_tools::registry::types::select_web_search_attribution_callback(None);
    assert!(
        absent.is_none(),
        "registry must not invent a session-primary fallback"
    );
    assert_eq!(resolved.route.instance_id(), "ws-prov");
    assert_eq!(resolved.route.operation_partition(), "web_search");
    let client = resolved.client().unwrap();
    assert_eq!(
        client.route_context().map(|r| r.instance_id()),
        Some("ws-prov")
    );
}

/// Production seam: HostFallback/legacy cannot inherit primary session attribution
/// for web-search tool registration.
#[test]
fn legacy_and_host_fallback_web_search_tool_cb_stays_absent() {
    let dir = tempdir().unwrap();
    let mgr = manager(IndexMap::new(), IndexMap::new(), "default", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    let mut resolved = resolve_auxiliary_route(AuxiliaryRouteInputs {
        explicit_pin_fail_closed: false,
        ..base_inputs(
            AuxiliaryPurpose::WebSearch,
            // Historical default web-search wire slug → legacy_compat when catalog-absent.
            crate::models::default_web_search_model(),
            &mgr,
            &frozen,
            &session,
            dir.path(),
            Some("jwt"),
        )
    })
    .unwrap();
    assert_eq!(resolved.kind, AuxiliaryRouteKind::LegacyCompat);
    assert!(!resolved.supports_route_bound_attribution());
    let am = Arc::new(crate::auth::AuthManager::new(
        dir.path(),
        crate::auth::GrokComConfig::default(),
    ));
    resolved.bind_attribution(Some(&am), Some("sid".into()));
    assert!(resolved.inference.attribution_callback.is_none());
    let tool_cb = web_search_tool_attribution_for_route(
        &am,
        Some("sid".into()),
        &resolved.route,
        resolved.kind,
    );
    assert!(tool_cb.is_none());
    // Even if a session primary exists, registry selection stays None.
    let selected =
        xai_grok_tools::registry::types::select_web_search_attribution_callback(tool_cb.as_ref());
    assert!(selected.is_none());
}

/// Production seam: Unverified BYOK with known instance retains exact callback.
#[test]
fn unverified_byok_retains_route_bound_attribution() {
    let dir = tempdir().unwrap();
    register_provider(
        dir.path(),
        "byok-prov",
        "openai_compatible",
        "https://byok.example/v1",
        Some("byok-key"),
    );
    let mut models = IndexMap::new();
    models.insert(
        "byok".into(),
        entry(
            "byok-wire",
            "byok-prov",
            "https://byok.example/v1",
            Some("byok-key"),
        ),
    );
    let mgr = manager(models, IndexMap::new(), "byok", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    let mut resolved = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::Compaction,
        "byok",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        None,
    ))
    .unwrap();
    // Temp home has no provider binding file → typically Unverified, not HostFallback.
    assert_ne!(resolved.route.authority(), RouteAuthority::HostFallback);
    assert_eq!(
        resolved.route.credential_route(),
        RouteCredentialRoute::ApiKey
    );
    assert!(resolved.supports_route_bound_attribution());
    let am = Arc::new(crate::auth::AuthManager::new(
        dir.path(),
        crate::auth::GrokComConfig::default(),
    ));
    resolved.bind_attribution(Some(&am), Some("byok-sid".into()));
    assert!(
        resolved.inference.attribution_callback.is_some(),
        "Unverified BYOK must keep exact-instance attribution, not lose it to is_authoritative()"
    );
}

/// Production seam: prompt + shell suggest purposes carry distinct operation partitions
/// and retain route on the live client (miss fails closed separately).
#[test]
fn prompt_and_shell_suggest_clients_retain_route() {
    let dir = tempdir().unwrap();
    register_provider(
        dir.path(),
        "suggest-p",
        "openai_compatible",
        "https://suggest.example/v1",
        Some("suggest-key"),
    );
    let mut models = IndexMap::new();
    models.insert(
        "suggest".into(),
        entry(
            "suggest-wire",
            "suggest-p",
            "https://suggest.example/v1",
            Some("suggest-key"),
        ),
    );
    let mgr = manager(models, IndexMap::new(), "suggest", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    for purpose in [
        AuxiliaryPurpose::PromptSuggest,
        AuxiliaryPurpose::ShellSuggest,
    ] {
        let resolved = resolve_auxiliary_route(base_inputs(
            purpose,
            "suggest",
            &mgr,
            &frozen,
            &session,
            dir.path(),
            None,
        ))
        .unwrap();
        let client = resolved.client().unwrap();
        assert_eq!(
            client.route_context().map(|r| r.operation_partition()),
            Some(purpose.operation_partition())
        );
        assert_eq!(resolved.upstream_model_id, "suggest-wire");
        assert_ne!(resolved.inference.base_url, session.base_url);
    }
}

/// Production seam: title pairing success uses upstream; soft-fallback keeps primary
/// wire model (never writes unresolved selection slug).
#[test]
fn title_pairing_and_soft_fallback_keep_primary_upstream() {
    let dir = tempdir().unwrap();
    register_provider(
        dir.path(),
        "title-p",
        "openai_compatible",
        "https://title.example/v1",
        Some("title-key"),
    );
    let mut models = IndexMap::new();
    models.insert(
        "title".into(),
        entry(
            "title-wire",
            "title-p",
            "https://title.example/v1",
            Some("title-key"),
        ),
    );
    let mgr = manager(models, IndexMap::new(), "title", dir.path());
    let frozen = frozen();
    let primary = session_cfg();

    // Success path: pairing retains route + upstream.
    let pair = SessionTitleSamplerPairing::for_session_new(base_inputs(
        AuxiliaryPurpose::SessionTitle,
        "title",
        &mgr,
        &frozen,
        &primary,
        dir.path(),
        None,
    ))
    .unwrap();
    assert_eq!(pair.model(), "title-wire");
    assert_eq!(pair.route.route.operation_partition(), "session_title");
    let client = pair.client().unwrap();
    assert_eq!(
        client.route_context().map(|r| r.instance_id()),
        Some("title-p")
    );

    // Soft-fallback path (mirrors build_summary_client): keep primary.model
    // AND exact primary route with SessionTitle purpose (F-PR6-5).
    let missing = SessionTitleSamplerPairing::for_session_new(base_inputs(
        AuxiliaryPurpose::SessionTitle,
        "missing-title-slug",
        &mgr,
        &frozen,
        &primary,
        dir.path(),
        None,
    ));
    assert!(missing.is_err());
    let mut fallback = session_title_soft_fallback_route(&frozen, &primary, "session-a");
    assert_eq!(fallback.upstream_model_id, "session-wire");
    assert_ne!(fallback.upstream_model_id, "missing-title-slug");
    assert_eq!(fallback.route.operation_partition(), "session_title");
    assert_eq!(fallback.route.instance_id(), frozen.instance_id());
    assert_eq!(
        fallback.route.binding_generation(),
        frozen.binding_generation()
    );
    let am = Arc::new(crate::auth::AuthManager::new(
        dir.path(),
        crate::auth::GrokComConfig::default(),
    ));
    fallback.bind_attribution(Some(&am), None);
    let client = fallback.client().unwrap();
    assert_eq!(
        client.route_context().map(|r| r.operation_partition()),
        Some("session_title")
    );
}

/// Production seam: media PDF/video purposes apply distinct operation partitions.
#[test]
fn media_pdf_and_video_purposes_apply_operation_partition() {
    let dir = tempdir().unwrap();
    register_provider(
        dir.path(),
        "vision-p",
        "openai_compatible",
        "https://vision.example/v1",
        Some("vision-key"),
    );
    let mut models = IndexMap::new();
    models.insert(
        "vision".into(),
        entry(
            "vision-wire",
            "vision-p",
            "https://vision.example/v1",
            Some("vision-key"),
        ),
    );
    let mgr = manager(models, IndexMap::new(), "vision", dir.path());
    let frozen = frozen();
    let session = session_cfg();

    let video = resolve_media_describe_route(
        base_inputs(
            AuxiliaryPurpose::MediaVideo,
            "vision",
            &mgr,
            &frozen,
            &session,
            dir.path(),
            None,
        ),
        AuxiliaryPurpose::MediaVideo,
    )
    .unwrap();
    assert_eq!(video.route.operation_partition(), "media_video");
    let video_client = video.client().unwrap();
    assert_eq!(
        video_client
            .route_context()
            .map(|r| r.operation_partition()),
        Some("media_video")
    );

    let pdf = resolve_media_describe_route(
        base_inputs(
            AuxiliaryPurpose::MediaPdf,
            "vision",
            &mgr,
            &frozen,
            &session,
            dir.path(),
            None,
        ),
        AuxiliaryPurpose::MediaPdf,
    )
    .unwrap();
    assert_eq!(pdf.route.operation_partition(), "media_pdf");
    let pdf_client = pdf.client().unwrap();
    assert_eq!(
        pdf_client.route_context().map(|r| r.operation_partition()),
        Some("media_pdf")
    );
}

/// Production seam: legacy_compat clears attribution (non-authoritative / no repair).
#[test]
fn legacy_compat_bind_attribution_is_non_authoritative_no_op() {
    let dir = tempdir().unwrap();
    let mgr = manager(IndexMap::new(), IndexMap::new(), "default", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    let mut resolved = resolve_auxiliary_route(AuxiliaryRouteInputs {
        explicit_pin_fail_closed: false,
        ..base_inputs(
            AuxiliaryPurpose::SessionTitle,
            "grok-legacy-x",
            &mgr,
            &frozen,
            &session,
            dir.path(),
            Some("jwt"),
        )
    })
    .unwrap();
    assert_eq!(resolved.kind, AuxiliaryRouteKind::LegacyCompat);
    resolved.bind_attribution(
        Some(&Arc::new(crate::auth::AuthManager::new(
            dir.path(),
            crate::auth::GrokComConfig::default(),
        ))),
        Some("sid".into()),
    );
    assert!(
        resolved.inference.attribution_callback.is_none(),
        "legacy_compat must not claim exact-account 401 repair identity"
    );
}

/// Catalog hit without credentials → exact CredentialUnavailable (not soft Ok).
#[test]
fn catalog_without_credentials_is_credential_unavailable() {
    let dir = tempdir().unwrap();
    let mut models = IndexMap::new();
    models.insert(
        "bare".into(),
        entry("bare", "p", "https://p.example/v1", None),
    );
    let mgr = manager(models, IndexMap::new(), "bare", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    let err = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::Compaction,
        "bare",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        None,
    ))
    .unwrap_err();
    assert!(matches!(
        err,
        AuxiliaryRouteError::CredentialUnavailable { selection }
        if selection == "bare"
    ));
}

/// Sibling accounts never share credentials across explicit pins.
#[test]
fn sibling_accounts_never_borrow_credentials() {
    let dir = tempdir().unwrap();
    register_provider(
        dir.path(),
        "a",
        "openai_compatible",
        "https://a.example/v1",
        Some("key-a"),
    );
    register_provider(
        dir.path(),
        "b",
        "openai_compatible",
        "https://b.example/v1",
        Some("key-b"),
    );
    let mut models = IndexMap::new();
    models.insert(
        "a:m".into(),
        entry("m", "a", "https://a.example/v1", Some("key-a")),
    );
    models.insert(
        "b:m".into(),
        entry("m", "b", "https://b.example/v1", Some("key-b")),
    );
    let mut providers = IndexMap::new();
    providers.insert(
        "a".into(),
        ModelProviderConfig {
            base_url: Some("https://a.example/v1".into()),
            ..Default::default()
        },
    );
    providers.insert(
        "b".into(),
        ModelProviderConfig {
            base_url: Some("https://b.example/v1".into()),
            ..Default::default()
        },
    );
    let mgr = manager(models, providers, "a:m", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    let a = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::Compaction,
        "a:m",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        None,
    ))
    .unwrap();
    let b = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::Compaction,
        "b:m",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        None,
    ))
    .unwrap();
    assert_eq!(a.inference.api_key.as_deref(), Some("key-a"));
    assert_eq!(b.inference.api_key.as_deref(), Some("key-b"));
    assert_ne!(a.route.instance_id(), b.route.instance_id());
    let a_client = a.client().unwrap();
    let b_client = b.client().unwrap();
    assert_ne!(
        a_client.route_context().map(|r| r.instance_id()),
        b_client.route_context().map(|r| r.instance_id())
    );
}

// --- Gate B F-PR6-1..6 production-seam coverage ---

fn frozen_xai() -> ProviderRouteContext {
    ProviderRouteContext::builder()
        .instance_id("xai")
        .provider_kind(RouteProviderKind::Xai)
        .api_surface(RouteApiSurface::OpenAiCompatibleSubset)
        .credential_route(RouteCredentialRoute::XaiSession)
        .binding_generation(7)
        .registry_generation(3)
        .authority(RouteAuthority::Authoritative)
        .origin_from_base_url("https://api.x.ai/v1")
        .model_partition("session-wire")
        .build()
        .unwrap()
}

/// F-PR6-1: SessionRecap @session inherit carries purpose partition + route client.
#[test]
fn session_recap_session_inherit_carries_route_and_purpose() {
    let dir = tempdir().unwrap();
    let mgr = manager(IndexMap::new(), IndexMap::new(), "session-a", dir.path());
    let frozen = frozen_xai();
    let session = session_cfg();
    let resolved = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::SessionRecap,
        SESSION_ROUTE_SENTINEL,
        &mgr,
        &frozen,
        &session,
        dir.path(),
        Some("jwt"),
    ))
    .unwrap();
    assert_eq!(resolved.kind, AuxiliaryRouteKind::SessionInherit);
    assert_eq!(resolved.purpose, AuxiliaryPurpose::SessionRecap);
    assert_eq!(resolved.route.operation_partition(), "session_recap");
    assert_eq!(resolved.route.instance_id(), "xai");
    assert_eq!(resolved.route.binding_generation(), 7);
    let client = resolved.client().unwrap();
    assert_eq!(
        client.route_context().map(|r| r.operation_partition()),
        Some("session_recap")
    );
}

/// F-PR6-2: MediaStt requires exact xAI session route; foreign session fails closed.
#[test]
fn media_stt_requires_xai_session_route_and_rejects_foreign() {
    let dir = tempdir().unwrap();
    let mgr = manager(IndexMap::new(), IndexMap::new(), "session-a", dir.path());
    let session = session_cfg();

    let xai = frozen_xai();
    let ok = resolve_media_stt_route(
        base_inputs(
            AuxiliaryPurpose::MediaStt,
            SESSION_ROUTE_SENTINEL,
            &mgr,
            &xai,
            &session,
            dir.path(),
            Some("jwt"),
        ),
        None,
    )
    .unwrap();
    assert_eq!(ok.purpose, AuxiliaryPurpose::MediaStt);
    assert_eq!(ok.route.operation_partition(), "media_stt");
    assert_eq!(ok.route.instance_id(), "xai");
    assert_eq!(ok.route.binding_generation(), 7);

    // Foreign OpenAI-compatible frozen route must not borrow xAI AuthManager bearer.
    let foreign = frozen(); // OpenAiCompatible account-a
    let err = resolve_media_stt_route(
        base_inputs(
            AuxiliaryPurpose::MediaStt,
            SESSION_ROUTE_SENTINEL,
            &mgr,
            &foreign,
            &session,
            dir.path(),
            Some("jwt"),
        ),
        None,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        AuxiliaryRouteError::ConstructionFailed { .. }
    ));

    // Arbitrary catalog audio pin fails closed.
    let err2 = resolve_media_stt_route(
        base_inputs(
            AuxiliaryPurpose::MediaStt,
            SESSION_ROUTE_SENTINEL,
            &mgr,
            &xai,
            &session,
            dir.path(),
            Some("jwt"),
        ),
        Some("some-catalog-audio-model"),
    )
    .unwrap_err();
    assert!(matches!(
        err2,
        AuxiliaryRouteError::ExplicitPinFailed { selection }
        if selection == "some-catalog-audio-model"
    ));
}

/// F-PR6-3: production freeze preserves instance/binding gens (not legacy_from_config).
#[test]
fn production_freeze_preserves_instance_and_binding_not_legacy_host() {
    let dir = tempdir().unwrap();
    register_provider(
        dir.path(),
        "account-a",
        "openai_compatible",
        "https://account-a.example/v1",
        Some("key-a"),
    );
    let mut models = IndexMap::new();
    models.insert(
        "session-a".into(),
        entry(
            "session-wire",
            "account-a",
            "https://account-a.example/v1",
            Some("key-a"),
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
    let mgr = manager(models, providers, "session-a", dir.path());
    let primary = session_cfg();
    let frozen = crate::session::route_context::resolve_for_models_manager_with_selection(
        &primary,
        &mgr,
        "session-a",
        Some(dir.path()),
    )
    .expect("provider route resolve");
    // Not a kind-only legacy freeze.
    assert_ne!(frozen.instance_id(), "openai");
    assert_ne!(frozen.instance_id(), "open_ai_compatible");
    // Production path may be Unverified without on-disk binding, but must not
    // be HostFallback legacy identity for a real catalog selection.
    assert_ne!(frozen.authority(), RouteAuthority::HostFallback);
    // @session inherit preserves generations from freeze.
    let inherited = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::WebSearch,
        SESSION_ROUTE_SENTINEL,
        &mgr,
        &frozen,
        &primary,
        dir.path(),
        None,
    ))
    .unwrap();
    assert_eq!(inherited.route.instance_id(), frozen.instance_id());
    assert_eq!(
        inherited.route.binding_generation(),
        frozen.binding_generation()
    );
    assert_eq!(inherited.route.operation_partition(), "web_search");
}

/// F-PR6-4: compaction purpose route client carries operation partition (prefire seam).
#[test]
fn compaction_route_client_operation_partition_for_two_pass_seam() {
    let dir = tempdir().unwrap();
    register_provider(
        dir.path(),
        "compact-p",
        "openai_compatible",
        "https://compact.example/v1",
        Some("compact-key"),
    );
    let mut models = IndexMap::new();
    models.insert(
        "compact".into(),
        entry(
            "compact-wire",
            "compact-p",
            "https://compact.example/v1",
            Some("compact-key"),
        ),
    );
    let mgr = manager(models, IndexMap::new(), "compact", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    let resolved = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::Compaction,
        "compact",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        None,
    ))
    .unwrap();
    let client = resolved.client().unwrap();
    assert_eq!(
        client.route_context().map(|r| r.operation_partition()),
        Some("compaction")
    );
    assert_eq!(
        client.route_context().map(|r| r.instance_id()),
        Some("compact-p")
    );
}

/// F-PR6-6: GoalEvaluator active-session path is @session inherit with purpose.
#[test]
fn goal_evaluator_session_inherit_carries_purpose() {
    let dir = tempdir().unwrap();
    let mgr = manager(IndexMap::new(), IndexMap::new(), "session-a", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    let resolved = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::GoalEvaluator,
        SESSION_ROUTE_SENTINEL,
        &mgr,
        &frozen,
        &session,
        dir.path(),
        Some("jwt"),
    ))
    .unwrap();
    assert_eq!(resolved.kind, AuxiliaryRouteKind::SessionInherit);
    assert_eq!(resolved.route.operation_partition(), "goal_evaluator");
    let client = resolved.client().unwrap();
    assert_eq!(
        client.route_context().map(|r| r.operation_partition()),
        Some("goal_evaluator")
    );
}

/// N5: SideQuestion + LazinessClassifier inherit frozen route with purpose.
#[test]
fn side_question_and_laziness_session_inherit_purposes() {
    let dir = tempdir().unwrap();
    let mgr = manager(IndexMap::new(), IndexMap::new(), "session-a", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    for purpose in [
        AuxiliaryPurpose::SideQuestion,
        AuxiliaryPurpose::LazinessClassifier,
    ] {
        let resolved = resolve_auxiliary_route(base_inputs(
            purpose,
            SESSION_ROUTE_SENTINEL,
            &mgr,
            &frozen,
            &session,
            dir.path(),
            Some("jwt"),
        ))
        .unwrap();
        assert_eq!(resolved.kind, AuxiliaryRouteKind::SessionInherit);
        assert_eq!(
            resolved.route.operation_partition(),
            purpose.operation_partition()
        );
        assert_eq!(
            resolved.route.binding_generation(),
            frozen.binding_generation()
        );
        assert!(resolved.client().is_ok());
    }
}
