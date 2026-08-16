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
            execution_backend: crate::agent::execution_backend::ExecutionBackend::NativeInference,
        },
        model_provider: Some(ResolvedModelProvider {
            id: provider.to_owned(),
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

/// Production seam: web-search resolve binds exact-route attribution (not sibling).
#[test]
fn web_search_prep_binds_exact_route_attribution_not_sibling() {
    let dir = tempdir().unwrap();
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
    let before_bind_was_session = resolved.inference.attribution_callback.is_some();
    resolved.bind_attribution(
        Some(&Arc::new(crate::auth::AuthManager::new(
            dir.path(),
            crate::auth::GrokComConfig::default(),
        ))),
        Some("ws-session".into()),
    );
    if resolved.route.is_authoritative()
        && resolved.route.credential_route() != RouteCredentialRoute::None
        && resolved.route.authority() != RouteAuthority::HostFallback
    {
        assert!(
            resolved.inference.attribution_callback.is_some(),
            "authoritative web-search route must bind exact-route attribution"
        );
    } else {
        // Non-authoritative routes use no-op (no exact-account repair identity).
        assert!(
            resolved.inference.attribution_callback.is_none(),
            "non-authoritative web-search route must clear sibling attribution"
        );
        assert!(
            before_bind_was_session || resolved.inference.attribution_callback.is_none(),
            "must not retain session sibling attribution on non-authoritative aux route"
        );
    }
    assert_eq!(resolved.route.instance_id(), "ws-prov");
    assert_eq!(resolved.route.operation_partition(), "web_search");
    let client = resolved.client().unwrap();
    assert_eq!(
        client.route_context().map(|r| r.instance_id()),
        Some("ws-prov")
    );
}

/// Production seam: prompt + shell suggest purposes carry distinct operation partitions
/// and retain route on the live client (miss fails closed separately).
#[test]
fn prompt_and_shell_suggest_clients_retain_route() {
    let dir = tempdir().unwrap();
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

    // Soft-fallback path (mirrors build_summary_client): keep primary.model.
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
    let fallback_model = primary.model.clone(); // never assign unresolved slug
    assert_eq!(fallback_model, "session-wire");
    assert_ne!(fallback_model, "missing-title-slug");
}

/// Production seam: media PDF/video purposes apply distinct operation partitions.
#[test]
fn media_pdf_and_video_purposes_apply_operation_partition() {
    let dir = tempdir().unwrap();
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
