//! PR6 auxiliary route boundary integration family (crate-private APIs).

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
}

#[test]
fn no_credential_route_fails_closed() {
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
        AuxiliaryRouteError::CredentialUnavailable { .. }
    ));
}

#[test]
fn namespaced_hijack_rejected_without_legacy_compat() {
    let dir = tempdir().unwrap();
    let mgr = manager(IndexMap::new(), IndexMap::new(), "default", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    let err = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::WebSearch,
        "openai_work:gpt-4o",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        Some("jwt"),
    ))
    .unwrap_err();
    assert!(matches!(
        err,
        AuxiliaryRouteError::NamespacedHijackRejected { .. }
    ));
}

#[test]
fn legacy_compat_is_non_authoritative_and_non_colliding() {
    let dir = tempdir().unwrap();
    let mgr = manager(IndexMap::new(), IndexMap::new(), "default", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    let resolved = resolve_auxiliary_route(AuxiliaryRouteInputs {
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
    assert_eq!(resolved.route.instance_id(), AUX_LEGACY_COMPAT_INSTANCE);
    assert_eq!(resolved.route.authority(), RouteAuthority::HostFallback);
    assert_eq!(
        resolved.route.credential_route(),
        RouteCredentialRoute::None
    );
    assert_eq!(resolved.route.binding_generation(), 0);
    assert_ne!(
        resolved.route.credential_route(),
        RouteCredentialRoute::XaiSession
    );
}

#[test]
fn frozen_session_inheritance_ignores_global_picker() {
    let dir = tempdir().unwrap();
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
    models.insert(
        "session-b".into(),
        entry(
            "other-wire",
            "account-b",
            "https://account-b.example/v1",
            Some("key-b"),
        ),
    );
    let mgr = manager(models, IndexMap::new(), "session-a", dir.path());
    mgr.set_current_model_id(acp::ModelId::new("session-b"));
    let frozen = frozen();
    let session = session_cfg();
    let resolved = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::Compaction,
        "@session",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        Some("jwt"),
    ))
    .unwrap();
    assert_eq!(resolved.kind, AuxiliaryRouteKind::SessionInherit);
    assert_eq!(resolved.route.instance_id(), "account-a");
    assert_eq!(resolved.route.binding_generation(), 4);
    assert_eq!(resolved.route.operation_partition(), "compaction");
}

#[test]
fn web_search_retains_exact_resolved_route() {
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
    let session = session_cfg();
    let resolved = resolve_auxiliary_route(base_inputs(
        AuxiliaryPurpose::WebSearch,
        "ws",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        None,
    ))
    .unwrap();
    assert_eq!(resolved.kind, AuxiliaryRouteKind::Explicit);
    assert_eq!(resolved.route.instance_id(), "ws-prov");
    assert_eq!(resolved.upstream_model_id, "ws-wire");
    assert_eq!(resolved.inference.api_key.as_deref(), Some("ws-key"));
    assert_eq!(resolved.route.operation_partition(), "web_search");
}

#[test]
fn title_and_media_boundaries_retain_route() {
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
    models.insert(
        "vision".into(),
        entry(
            "vision-wire",
            "vision-p",
            "https://vision.example/v1",
            Some("vision-key"),
        ),
    );
    let mgr = manager(models, IndexMap::new(), "title", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    let title = SessionTitleSamplerPairing::for_session_new(base_inputs(
        AuxiliaryPurpose::SessionTitle,
        "title",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        None,
    ))
    .unwrap();
    assert_eq!(title.model(), "title-wire");
    assert_eq!(title.route.route.instance_id(), "title-p");
    assert_eq!(title.route.route.operation_partition(), "session_title");

    let media = resolve_media_describe_route(base_inputs(
        AuxiliaryPurpose::MediaDescribe,
        "vision",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        None,
    ))
    .unwrap();
    assert_eq!(media.upstream_model_id, "vision-wire");
    assert_eq!(media.route.instance_id(), "vision-p");
    assert_eq!(media.route.operation_partition(), "media_describe");
}

#[test]
fn explicit_media_pin_failure_is_closed() {
    let dir = tempdir().unwrap();
    let mgr = manager(IndexMap::new(), IndexMap::new(), "default", dir.path());
    let frozen = frozen();
    let session = session_cfg();
    let err = resolve_media_describe_route(base_inputs(
        AuxiliaryPurpose::MediaDescribe,
        "missing-vision",
        &mgr,
        &frozen,
        &session,
        dir.path(),
        Some("jwt"),
    ))
    .unwrap_err();
    assert!(matches!(
        err,
        AuxiliaryRouteError::ExplicitPinFailed { .. } | AuxiliaryRouteError::Missing { .. }
    ));
}
