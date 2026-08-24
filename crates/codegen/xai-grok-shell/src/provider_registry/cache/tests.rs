//! Production-boundary tests for incarnation-safe provider cache storage.

use super::fs::{CAPABILITIES_FILE, CATALOG_FILE, PROVIDER_CACHES_DIR, STATE_FILE, TXN_FILE};
use super::*;
use crate::provider_registry::id::ProviderId;
use crate::provider_registry::instance::{
    ApiSurface, CredentialRoute, ProviderIncarnation, ProviderKind,
};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

fn pid(s: &str) -> ProviderId {
    ProviderId::new(s).unwrap()
}

fn inc(s: &str) -> ProviderIncarnation {
    ProviderIncarnation::new(s).unwrap()
}

fn bind(s: &str) -> CredentialBindingId {
    CredentialBindingId::new(s).unwrap()
}

fn identity(
    id: &str,
    incarnation: &str,
    kind: ProviderKind,
    surface: ApiSurface,
    route: CredentialRoute,
    origin: &str,
    binding: &str,
    org_fp: &str,
) -> ProviderCacheIdentity {
    ProviderCacheIdentity::new(
        pid(id),
        inc(incarnation),
        kind,
        surface,
        route,
        origin,
        org_fp,
        bind(binding),
    )
    .expect("valid test identity")
}

fn sample_catalog(identity: &ProviderCacheIdentity, model: &str) -> CatalogCacheEntry {
    CatalogCacheEntry {
        version: CATALOG_CACHE_VERSION,
        provider_id: identity.instance_id.as_str().into(),
        origin: CacheOrigin::Live,
        base_url_origin: identity.endpoint_origin.clone(),
        fetched_at_unix: 1_700_000_000,
        models: vec![serde_json::json!({"id": model})],
        baseline_version: None,
        incarnation: Some(identity.incarnation.clone()),
        provider_kind: Some(identity.kind),
        api_surface: Some(identity.api_surface),
        credential_route: Some(identity.credential_route),
        credential_binding_id: Some(identity.credential_binding_id.clone()),
        org_project_fingerprint: Some(identity.org_project_fingerprint.clone()),
        catalog_generation: 0,
        lifecycle_generation: None,
    }
}

fn sample_capability(identity: &ProviderCacheIdentity, baseline: &str) -> CapabilityCacheEntry {
    CapabilityCacheEntry {
        version: CAPABILITY_CACHE_VERSION,
        provider_id: identity.instance_id.as_str().into(),
        origin: CacheOrigin::Probe,
        base_url_origin: identity.endpoint_origin.clone(),
        baseline_version: baseline.into(),
        fetched_at_unix: 10,
        capabilities: serde_json::json!({"chat": true}),
        evidence: None,
        incarnation: Some(identity.incarnation.clone()),
        provider_kind: Some(identity.kind),
        api_surface: Some(identity.api_surface),
        credential_route: Some(identity.credential_route),
        credential_binding_id: Some(identity.credential_binding_id.clone()),
        org_project_fingerprint: Some(identity.org_project_fingerprint.clone()),
        capability_generation: 0,
        lifecycle_generation: None,
    }
}

const INC_A: &str = "11111111-1111-1111-1111-111111111111";
const INC_B: &str = "22222222-2222-2222-2222-222222222222";
const BIND_A: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const BIND_B: &str = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

#[test]
fn sibling_isolation_same_kind_origin_different_incarnation_binding() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let a = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    );
    let b = identity(
        "local_openai",
        INC_B,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_B,
        "",
    );
    ProviderCacheStore::store_catalog(home, &a, &sample_catalog(&a, "gpt-a")).unwrap();
    ProviderCacheStore::store_catalog(home, &b, &sample_catalog(&b, "gpt-b")).unwrap();
    assert_eq!(
        ProviderCacheStore::load_catalog(home, &a)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "gpt-a"
    );
    assert_eq!(
        ProviderCacheStore::load_catalog(home, &b)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "gpt-b"
    );
}

#[test]
fn rejects_origin_kind_surface_route_incarnation_binding_org_mismatch() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let base = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        &org_project_fingerprint(Some("org"), Some("proj")),
    );
    ProviderCacheStore::store_catalog(home, &base, &sample_catalog(&base, "m")).unwrap();

    let mut origin = base.clone();
    origin.endpoint_origin = "https://evil.example".into();
    assert!(matches!(
        ProviderCacheStore::load_catalog(home, &origin),
        Err(CacheValidationError::OriginMismatch)
    ));

    let mut kind = base.clone();
    kind.kind = ProviderKind::OpenRouter;
    assert!(matches!(
        ProviderCacheStore::load_catalog(home, &kind),
        Err(CacheValidationError::KindMismatch)
    ));

    let mut surface = base.clone();
    surface.api_surface = ApiSurface::OpenRouterNative;
    assert!(matches!(
        ProviderCacheStore::load_catalog(home, &surface),
        Err(CacheValidationError::SurfaceMismatch)
    ));

    let mut route = base.clone();
    route.credential_route = CredentialRoute::ChatGptOauth;
    assert!(matches!(
        ProviderCacheStore::load_catalog(home, &route),
        Err(CacheValidationError::RouteMismatch)
    ));

    let mut incarnation = base.clone();
    incarnation.incarnation = inc(INC_B);
    assert!(matches!(
        ProviderCacheStore::load_catalog(home, &incarnation),
        Err(CacheValidationError::IncarnationMismatch)
    ));

    let mut binding = base.clone();
    binding.credential_binding_id = bind(BIND_B);
    assert!(matches!(
        ProviderCacheStore::load_catalog(home, &binding),
        Err(CacheValidationError::BindingMismatch)
    ));

    let mut org = base.clone();
    org.org_project_fingerprint = org_project_fingerprint(Some("other"), Some("proj"));
    assert!(matches!(
        ProviderCacheStore::load_catalog(home, &org),
        Err(CacheValidationError::OrgProjectMismatch)
    ));
}

#[test]
fn credential_binding_rotation_invalidates_only_one_instance() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let a = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    );
    let sibling = identity(
        "openrouter",
        INC_B,
        ProviderKind::OpenRouter,
        ApiSurface::OpenRouterNative,
        CredentialRoute::ApiKey,
        "https://openrouter.ai",
        BIND_B,
        "",
    );
    ProviderCacheStore::store_catalog(home, &a, &sample_catalog(&a, "a")).unwrap();
    ProviderCacheStore::store_catalog(home, &sibling, &sample_catalog(&sibling, "s")).unwrap();

    let mut rotated = a.clone();
    rotated.credential_binding_id = bind(BIND_B);
    assert!(matches!(
        ProviderCacheStore::load_catalog(home, &rotated),
        Err(CacheValidationError::BindingMismatch)
    ));
    assert!(
        ProviderCacheStore::load_catalog(home, &sibling)
            .unwrap()
            .is_some()
    );

    ProviderCacheStore::store_catalog(home, &rotated, &sample_catalog(&rotated, "rotated"))
        .unwrap();
    assert_eq!(
        ProviderCacheStore::load_catalog(home, &rotated)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "rotated"
    );
    assert!(matches!(
        ProviderCacheStore::load_catalog(home, &a),
        Err(CacheValidationError::BindingMismatch)
    ));
    assert!(
        ProviderCacheStore::load_catalog(home, &sibling)
            .unwrap()
            .is_some()
    );
}

#[test]
fn remove_recreate_same_id_cannot_bind_old_cache() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let first = identity(
        "local_vllm",
        INC_A,
        ProviderKind::OpenAiCompatible,
        ApiSurface::OpenAiCompatibleSubset,
        CredentialRoute::ApiKey,
        "http://127.0.0.1:8000",
        BIND_A,
        "",
    );
    ProviderCacheStore::store_catalog(home, &first, &sample_catalog(&first, "old")).unwrap();
    remove_all_provider_caches(home, &first.instance_id).unwrap();

    let second = identity(
        "local_vllm",
        INC_B,
        ProviderKind::OpenAiCompatible,
        ApiSurface::OpenAiCompatibleSubset,
        CredentialRoute::ApiKey,
        "http://127.0.0.1:8000",
        BIND_B,
        "",
    );
    assert!(
        ProviderCacheStore::load_catalog(home, &second)
            .unwrap()
            .is_none()
    );
    ProviderCacheStore::store_catalog(home, &second, &sample_catalog(&second, "new")).unwrap();
    assert_eq!(
        ProviderCacheStore::load_catalog(home, &second)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "new"
    );
    assert!(matches!(
        ProviderCacheStore::load_catalog(home, &first),
        Err(CacheValidationError::IncarnationMismatch)
            | Err(CacheValidationError::BindingMismatch)
            | Ok(None)
    ));
}

#[test]
fn copy_only_legacy_builtin_import_leaves_legacy_bytes_unchanged() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let legacy_path = home.join("openai_models_cache.json");
    let legacy_bytes = serde_json::to_vec(&serde_json::json!({
        "version": 2,
        "models": [{"id": "gpt-legacy", "label": "Legacy"}]
    }))
    .unwrap();
    fs::write(&legacy_path, &legacy_bytes).unwrap();

    let id_openai = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    );
    let imported = ProviderCacheStore::try_import_legacy_builtin(home, &id_openai)
        .unwrap()
        .expect("import");
    assert_eq!(imported.origin, CacheOrigin::LegacyMigration);
    assert_eq!(imported.models[0]["id"], "gpt-legacy");
    assert_eq!(fs::read(&legacy_path).unwrap(), legacy_bytes);

    let sibling = identity(
        "local_openai",
        INC_B,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_B,
        "",
    );
    assert!(
        ProviderCacheStore::try_import_legacy_builtin(home, &sibling)
            .unwrap()
            .is_none()
    );
    assert_eq!(fs::read(&legacy_path).unwrap(), legacy_bytes);
}

#[test]
fn partial_compat_store_refuses_over_authoritative_state() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let id = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    );
    ProviderCacheStore::store_catalog(home, &id, &sample_catalog(&id, "authoritative")).unwrap();
    let partial = CatalogCacheEntry {
        version: CATALOG_CACHE_VERSION,
        provider_id: "openai".into(),
        origin: CacheOrigin::Live,
        base_url_origin: "https://api.openai.com".into(),
        fetched_at_unix: 1,
        models: vec![serde_json::json!({"id": "weaker"})],
        baseline_version: None,
        incarnation: None,
        provider_kind: None,
        api_surface: None,
        credential_route: None,
        credential_binding_id: None,
        org_project_fingerprint: None,
        catalog_generation: 0,
        lifecycle_generation: None,
    };
    assert!(CatalogCacheStore::store(home, &partial).is_err());
    assert_eq!(
        ProviderCacheStore::load_catalog(home, &id)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "authoritative"
    );
}

#[test]
fn incarnation_supersede_clears_other_payload_and_compat_capabilities() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let first = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    );
    ProviderCacheStore::store_catalog(home, &first, &sample_catalog(&first, "cat-a")).unwrap();
    ProviderCacheStore::store_capabilities(home, &first, &sample_capability(&first, "base-a"))
        .unwrap();

    let second = identity(
        "openai",
        INC_B,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_B,
        "",
    );
    ProviderCacheStore::store_catalog(home, &second, &sample_catalog(&second, "cat-b")).unwrap();
    assert!(matches!(
        CapabilityCacheStore::load(home, &pid("openai"), "https://api.openai.com", "base-a"),
        Err(CacheValidationError::IncarnationMismatch)
            | Err(CacheValidationError::BindingMismatch)
            | Ok(None)
    ));
    assert!(
        ProviderCacheStore::load_capabilities(home, &second, "base-a")
            .unwrap()
            .is_none()
    );
}

#[test]
fn tombstone_blocks_same_identity_store() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let id = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    );
    ProviderCacheStore::store_catalog(home, &id, &sample_catalog(&id, "m")).unwrap();
    ProviderCacheStore::tombstone(home, &id).unwrap();
    assert!(matches!(
        ProviderCacheStore::store_catalog(home, &id, &sample_catalog(&id, "stale")),
        Err(CacheValidationError::Tombstoned)
    ));
    let next = identity(
        "openai",
        INC_B,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_B,
        "",
    );
    ProviderCacheStore::store_catalog(home, &next, &sample_catalog(&next, "fresh")).unwrap();
    assert_eq!(
        ProviderCacheStore::load_catalog(home, &next)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "fresh"
    );
}

#[test]
fn legacy_import_preserves_no_state_compat_catalog() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    fs::write(
        home.join("openai_models_cache.json"),
        serde_json::to_vec(&serde_json::json!({
            "version": 2,
            "models": [{"id": "legacy-only"}]
        }))
        .unwrap(),
    )
    .unwrap();
    let partial = CatalogCacheEntry {
        version: CATALOG_CACHE_VERSION,
        provider_id: "openai".into(),
        origin: CacheOrigin::Live,
        base_url_origin: "https://api.openai.com".into(),
        fetched_at_unix: 42,
        models: vec![serde_json::json!({"id": "compat-newer"})],
        baseline_version: None,
        incarnation: None,
        provider_kind: None,
        api_surface: None,
        credential_route: None,
        credential_binding_id: None,
        org_project_fingerprint: None,
        catalog_generation: 0,
        lifecycle_generation: None,
    };
    CatalogCacheStore::store(home, &partial).unwrap();
    let id = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    );
    let got = ProviderCacheStore::try_import_legacy_builtin(home, &id)
        .unwrap()
        .unwrap();
    assert_eq!(got.models[0]["id"], "compat-newer");
    assert_eq!(got.origin, CacheOrigin::Live);
}

#[cfg(unix)]
#[test]
fn rejects_symlink_attacks_at_sensitive_levels() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let id_openai = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    );
    ProviderCacheStore::store_catalog(home, &id_openai, &sample_catalog(&id_openai, "m")).unwrap();
    let inst = home.join(PROVIDER_CACHES_DIR).join("openai");
    let outside = home.join("outside_target");
    fs::write(&outside, b"pwned").unwrap();
    let catalog = inst.join(CATALOG_FILE);
    fs::remove_file(&catalog).unwrap();
    std::os::unix::fs::symlink(&outside, &catalog).unwrap();
    assert!(ProviderCacheStore::load_catalog(home, &id_openai).is_err());
}

#[test]
fn rejects_corrupt_and_oversized_files() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let id_openai = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    );
    ProviderCacheStore::store_catalog(home, &id_openai, &sample_catalog(&id_openai, "m")).unwrap();
    let catalog = home
        .join(PROVIDER_CACHES_DIR)
        .join("openai")
        .join(CATALOG_FILE);
    fs::write(&catalog, b"{not-json").unwrap();
    assert!(matches!(
        ProviderCacheStore::load_catalog(home, &id_openai),
        Err(CacheValidationError::Corrupt(_))
    ));

    ProviderCacheStore::store_catalog(home, &id_openai, &sample_catalog(&id_openai, "m")).unwrap();
    let oversized = vec![b'x'; 8 * 1024 * 1024 + 64];
    fs::write(&catalog, &oversized).unwrap();
    assert!(ProviderCacheStore::load_catalog(home, &id_openai).is_err());
}

#[test]
fn concurrent_same_instance_writers_serialize_siblings_independent() {
    let dir = TempDir::new().unwrap();
    let home = Arc::new(dir.path().to_path_buf());
    let a = Arc::new(identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    ));
    let b = Arc::new(identity(
        "openrouter",
        INC_B,
        ProviderKind::OpenRouter,
        ApiSurface::OpenRouterNative,
        CredentialRoute::ApiKey,
        "https://openrouter.ai",
        BIND_B,
        "",
    ));
    ProviderCacheStore::store_catalog(&home, &a, &sample_catalog(&a, "seed-a")).unwrap();
    ProviderCacheStore::store_catalog(&home, &b, &sample_catalog(&b, "seed-b")).unwrap();
    let barrier = Arc::new(Barrier::new(4));
    let mut handles = Vec::new();
    for i in 0..2 {
        let home = Arc::clone(&home);
        let a = Arc::clone(&a);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            for round in 0..20 {
                let mut entry = sample_catalog(&a, &format!("a-{i}-{round}"));
                entry.fetched_at_unix = round as u64;
                ProviderCacheStore::store_catalog(&home, &a, &entry).unwrap();
            }
        }));
    }
    for i in 0..2 {
        let home = Arc::clone(&home);
        let b = Arc::clone(&b);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            for round in 0..20 {
                let mut entry = sample_catalog(&b, &format!("b-{i}-{round}"));
                entry.fetched_at_unix = round as u64;
                ProviderCacheStore::store_catalog(&home, &b, &entry).unwrap();
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let loaded_a = ProviderCacheStore::load_catalog(&home, &a)
        .unwrap()
        .unwrap();
    let loaded_b = ProviderCacheStore::load_catalog(&home, &b)
        .unwrap()
        .unwrap();
    assert!(loaded_a.models[0]["id"].as_str().unwrap().starts_with("a-"));
    assert!(loaded_b.models[0]["id"].as_str().unwrap().starts_with("b-"));
}

#[test]
fn crash_failpoints_recover_or_fail_closed() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let id_openai = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    );

    ProviderCacheStore::store_catalog(home, &id_openai, &sample_catalog(&id_openai, "seed"))
        .unwrap();
    {
        let _guard = super::fault::FaultGuard::arm(ProviderCacheTxnFault::AfterJournalFsync);
        assert!(
            ProviderCacheStore::store_catalog(
                home,
                &id_openai,
                &sample_catalog(&id_openai, "post-journal")
            )
            .is_err()
        );
    }
    assert_eq!(
        ProviderCacheStore::load_catalog(home, &id_openai)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "post-journal"
    );

    {
        let _guard = super::fault::FaultGuard::arm(ProviderCacheTxnFault::AfterStateRename);
        assert!(
            ProviderCacheStore::store_catalog(
                home,
                &id_openai,
                &sample_catalog(&id_openai, "post-state")
            )
            .is_err()
        );
    }
    assert_eq!(
        ProviderCacheStore::load_catalog(home, &id_openai)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "post-state"
    );
    ProviderCacheStore::store_catalog(home, &id_openai, &sample_catalog(&id_openai, "final"))
        .unwrap();
    assert_eq!(
        ProviderCacheStore::load_catalog(home, &id_openai)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "final"
    );
}

fn partial_catalog_entry(id: &str, origin: &str, model: &str) -> CatalogCacheEntry {
    CatalogCacheEntry {
        version: CATALOG_CACHE_VERSION,
        provider_id: id.into(),
        origin: CacheOrigin::Live,
        base_url_origin: origin.into(),
        fetched_at_unix: 1,
        models: vec![serde_json::json!({"id": model})],
        baseline_version: None,
        incarnation: None,
        provider_kind: None,
        api_surface: None,
        credential_route: None,
        credential_binding_id: None,
        org_project_fingerprint: None,
        catalog_generation: 0,
        lifecycle_generation: None,
    }
}

fn partial_capability_entry(
    id: &str,
    origin: &str,
    baseline: &str,
    chat: bool,
) -> CapabilityCacheEntry {
    CapabilityCacheEntry {
        version: CAPABILITY_CACHE_VERSION,
        provider_id: id.into(),
        origin: CacheOrigin::Probe,
        base_url_origin: origin.into(),
        baseline_version: baseline.into(),
        fetched_at_unix: 2,
        capabilities: serde_json::json!({"chat": chat}),
        evidence: None,
        incarnation: None,
        provider_kind: None,
        api_surface: None,
        credential_route: None,
        credential_binding_id: None,
        org_project_fingerprint: None,
        capability_generation: 0,
        lifecycle_generation: None,
    }
}

#[test]
fn partial_catalog_recovery_preserves_sibling_capabilities() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let origin = "http://127.0.0.1:8000";
    let pid_local = pid("local_a");

    CatalogCacheStore::store(home, &partial_catalog_entry("local_a", origin, "cat-seed")).unwrap();
    CapabilityCacheStore::store(
        home,
        &partial_capability_entry("local_a", origin, "base-1", true),
    )
    .unwrap();
    let caps_before = fs::read(
        home.join(PROVIDER_CACHES_DIR)
            .join("local_a")
            .join(CAPABILITIES_FILE),
    )
    .unwrap();

    {
        let _guard = super::fault::FaultGuard::arm(ProviderCacheTxnFault::AfterJournalFsync);
        assert!(
            CatalogCacheStore::store(
                home,
                &partial_catalog_entry("local_a", origin, "cat-post-journal"),
            )
            .is_err()
        );
    }
    assert_eq!(
        CatalogCacheStore::load(home, &pid_local, origin)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "cat-post-journal"
    );
    assert_eq!(
        fs::read(
            home.join(PROVIDER_CACHES_DIR)
                .join("local_a")
                .join(CAPABILITIES_FILE),
        )
        .unwrap(),
        caps_before
    );

    CatalogCacheStore::store(home, &partial_catalog_entry("local_a", origin, "cat-final")).unwrap();
    assert_eq!(
        CatalogCacheStore::load(home, &pid_local, origin)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "cat-final"
    );
    assert_eq!(
        CapabilityCacheStore::load(home, &pid_local, origin, "base-1")
            .unwrap()
            .unwrap()
            .capabilities["chat"],
        true
    );
}

#[test]
fn partial_capabilities_recovery_preserves_sibling_catalog() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let origin = "http://127.0.0.1:9000";
    let pid_local = pid("local_b");

    CatalogCacheStore::store(
        home,
        &partial_catalog_entry("local_b", origin, "cat-sibling"),
    )
    .unwrap();
    CapabilityCacheStore::store(
        home,
        &partial_capability_entry("local_b", origin, "base-x", false),
    )
    .unwrap();
    let catalog_before = fs::read(
        home.join(PROVIDER_CACHES_DIR)
            .join("local_b")
            .join(CATALOG_FILE),
    )
    .unwrap();

    {
        let _guard = super::fault::FaultGuard::arm(ProviderCacheTxnFault::AfterJournalFsync);
        assert!(
            CapabilityCacheStore::store(
                home,
                &partial_capability_entry("local_b", origin, "base-x", true),
            )
            .is_err()
        );
    }
    assert_eq!(
        CapabilityCacheStore::load(home, &pid_local, origin, "base-x")
            .unwrap()
            .unwrap()
            .capabilities["chat"],
        true
    );
    assert_eq!(
        fs::read(
            home.join(PROVIDER_CACHES_DIR)
                .join("local_b")
                .join(CATALOG_FILE),
        )
        .unwrap(),
        catalog_before
    );
}

#[test]
fn recovery_missing_temp_preserves_lkg() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let id = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    );
    ProviderCacheStore::store_catalog(home, &id, &sample_catalog(&id, "lkg")).unwrap();
    let inst = home.join(PROVIDER_CACHES_DIR).join("openai");
    let catalog_bytes = fs::read(inst.join(CATALOG_FILE)).unwrap();
    let state_bytes = fs::read(inst.join(STATE_FILE)).unwrap();
    let catalog_hash = {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(&catalog_bytes))
    };
    let state_hash = {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(&state_bytes))
    };
    let marker = serde_json::json!({
        "version": 1,
        "catalog_tmp": ".catalog.json.1.deadbeefdeadbeefdeadbeefdeadbeef.tmp",
        "catalog_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "state_sha256": state_hash,
        "previous_catalog_sha256": catalog_hash,
        "previous_state_sha256": state_hash,
        "partial_compat": false
    });
    fs::write(
        inst.join(TXN_FILE),
        serde_json::to_vec_pretty(&marker).unwrap(),
    )
    .unwrap();
    assert_eq!(
        ProviderCacheStore::load_catalog(home, &id)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "lkg"
    );
    assert!(!inst.join(TXN_FILE).exists());
}

#[test]
fn compat_catalog_api_round_trip_and_sibling_isolation() {
    let dir = TempDir::new().unwrap();
    let a = pid("local_a");
    let b = pid("local_b");
    let entry = CatalogCacheEntry {
        version: CATALOG_CACHE_VERSION,
        provider_id: a.as_str().into(),
        origin: CacheOrigin::Live,
        base_url_origin: "http://127.0.0.1:8000".into(),
        fetched_at_unix: 1,
        models: vec![serde_json::json!({"id": "m"})],
        baseline_version: None,
        incarnation: None,
        provider_kind: None,
        api_surface: None,
        credential_route: None,
        credential_binding_id: None,
        org_project_fingerprint: None,
        catalog_generation: 0,
        lifecycle_generation: None,
    };
    CatalogCacheStore::store(dir.path(), &entry).unwrap();
    assert!(
        CatalogCacheStore::load(dir.path(), &a, "http://127.0.0.1:8000")
            .unwrap()
            .is_some()
    );
    assert!(
        CatalogCacheStore::load(dir.path(), &b, "http://127.0.0.1:8000")
            .unwrap()
            .is_none()
    );
}

#[test]
fn remove_instance_uses_dirfd_and_does_not_touch_sibling() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let a = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        "",
    );
    let b = identity(
        "openrouter",
        INC_B,
        ProviderKind::OpenRouter,
        ApiSurface::OpenRouterNative,
        CredentialRoute::ApiKey,
        "https://openrouter.ai",
        BIND_B,
        "",
    );
    ProviderCacheStore::store_catalog(home, &a, &sample_catalog(&a, "a")).unwrap();
    ProviderCacheStore::store_catalog(home, &b, &sample_catalog(&b, "b")).unwrap();
    remove_all_provider_caches(home, &a.instance_id).unwrap();
    assert!(
        ProviderCacheStore::load_catalog(home, &a)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        ProviderCacheStore::load_catalog(home, &b)
            .unwrap()
            .unwrap()
            .models[0]["id"],
        "b"
    );
}

#[test]
fn state_json_is_secret_free() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    let org_fp = org_project_fingerprint(Some("secret-org"), Some("secret-proj"));
    let id_openai = identity(
        "openai",
        INC_A,
        ProviderKind::OpenAi,
        ApiSurface::OpenAiPlatform,
        CredentialRoute::ApiKey,
        "https://api.openai.com",
        BIND_A,
        &org_fp,
    );
    ProviderCacheStore::store_catalog(home, &id_openai, &sample_catalog(&id_openai, "m")).unwrap();
    let raw = fs::read_to_string(
        home.join(PROVIDER_CACHES_DIR)
            .join("openai")
            .join(STATE_FILE),
    )
    .unwrap();
    assert!(!raw.contains("secret-org"));
    assert!(!raw.contains("secret-proj"));
    assert!(raw.contains(&org_fp));
}

fn _touch_caps_path(home: &Path) {
    let _ = home.join(CAPABILITIES_FILE);
}
