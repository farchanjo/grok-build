//! Production-boundary tests for paginated catalog adapters and atomic publication.
//!
//! Local mock servers and temporary homes only. Gate env mutations use the
//! shared process lock.

use super::*;
use crate::provider_registry::{
    ApiSurface, CredentialBindingId, CredentialRoute, MULTI_ACCOUNT_ROLLOUT_ENV,
    ProviderCacheIdentity, ProviderCacheStore, ProviderIncarnation, ProviderKind,
    multi_account_rollout_enabled, multi_account_rollout_env_lock,
};
use indexmap::IndexMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

struct EnvRestore {
    key: &'static str,
    previous: Option<String>,
}

impl EnvRestore {
    fn capture(key: &'static str) -> Self {
        Self {
            key,
            previous: std::env::var(key).ok(),
        }
    }
    fn set(&self, value: &str) {
        unsafe { std::env::set_var(self.key, value) };
    }
    fn clear(&self) {
        unsafe { std::env::remove_var(self.key) };
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        match &self.previous {
            Some(v) => unsafe { std::env::set_var(self.key, v) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

fn inc() -> ProviderIncarnation {
    ProviderIncarnation::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap()
}

fn bind(n: u8) -> CredentialBindingId {
    let s = format!("11111111-2222-3333-4444-55555555555{n}");
    CredentialBindingId::new(s).unwrap()
}

fn identity(
    id: &str,
    kind: ProviderKind,
    origin: &str,
    binding: u8,
    built_in: bool,
) -> CatalogAccountIdentity {
    CatalogAccountIdentity {
        instance_id: crate::provider_registry::ProviderId::new(id).unwrap(),
        kind,
        api_surface: match kind {
            ProviderKind::OpenRouter => ApiSurface::OpenRouterNative,
            _ => ApiSurface::OpenAiPlatform,
        },
        credential_route: CredentialRoute::ApiKey,
        endpoint_origin: origin.to_owned(),
        org_project_fingerprint: String::new(),
        incarnation: inc(),
        credential_binding_id: bind(binding),
        is_built_in_compatibility: built_in,
    }
}

fn cred(binding: u8, token: &str) -> CatalogCredential {
    CatalogCredential::new(bind(binding), 1, token).unwrap()
}

/// Scripted HTTP server that records full request lines and Authorization.
struct RecordedHop {
    path_contains: String,
    status: u16,
    body: String,
    /// Optional delay before responding (deadline tests).
    delay: Duration,
}

fn spawn_scripted_server(
    responses: Vec<RecordedHop>,
) -> (
    String,
    Arc<std::sync::Mutex<Vec<String>>>,
    std::thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let recorded = Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded_c = recorded.clone();
    let handle = std::thread::spawn(move || {
        for hop in responses {
            let Ok((mut stream, _)) = listener.accept() else {
                break;
            };
            let mut buf = [0u8; 16384];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).into_owned();
            recorded_c.lock().unwrap().push(req.clone());
            assert!(
                req.contains(&hop.path_contains) || hop.path_contains.is_empty(),
                "expected path containing {:?}\n{req}",
                hop.path_contains
            );
            // Every authenticated hop must carry Authorization: Bearer …
            assert!(
                req.to_ascii_lowercase().contains("authorization: bearer "),
                "missing Authorization bearer on hop:\n{req}"
            );
            if hop.delay > Duration::ZERO {
                std::thread::sleep(hop.delay);
            }
            let reason = if hop.status == 200 { "OK" } else { "ERR" };
            let _ = write!(
                stream,
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                hop.status,
                reason,
                hop.body.len(),
                hop.body
            );
        }
    });
    (format!("http://{addr}"), recorded, handle)
}

fn hop(path: &str, status: u16, body: &str) -> RecordedHop {
    RecordedHop {
        path_contains: path.into(),
        status,
        body: body.into(),
        delay: Duration::ZERO,
    }
}

// ── 1. Four accounts ───────────────────────────────────────────────────────

#[tokio::test]
async fn four_accounts_distinct_canonical_ids_gate_hides_additional() {
    // Shared process lock with gate.rs tests — hold across env set/restore
    // and the full refresh/publish that reads multi_account_rollout_enabled().
    let _gate = multi_account_rollout_env_lock();
    let _restore = EnvRestore::capture(MULTI_ACCOUNT_ROLLOUT_ENV);
    _restore.set("1");
    assert!(multi_account_rollout_enabled());

    let home = tempfile::tempdir().unwrap();
    let publisher = Arc::new(CatalogPublisher::new());
    let coord = CatalogRefreshCoordinator::with_fixed_registry(home.path(), publisher.clone(), 1);

    let (oa1, rec1, h1) = spawn_scripted_server(vec![hop(
        "/models",
        200,
        r#"{"data":[{"id":"shared-model"}]}"#,
    )]);
    let (oa2, _, h2) = spawn_scripted_server(vec![hop(
        "/models",
        200,
        r#"{"data":[{"id":"shared-model"}]}"#,
    )]);
    let (or1, _, h3) = spawn_scripted_server(vec![hop(
        "/models",
        200,
        r#"{"data":[{"id":"shared-model","name":"Shared","supported_parameters":["tools"]}]}"#,
    )]);
    let (or2, _, h4) = spawn_scripted_server(vec![hop(
        "/models",
        200,
        r#"{"data":[{"id":"shared-model","name":"Shared","supported_parameters":["tools"]}]}"#,
    )]);

    let targets = vec![
        CatalogRefreshTarget {
            identity: identity("openai", ProviderKind::OpenAi, &oa1, 1, true),
            models_list_url: format!("{oa1}/models"),
            credential: cred(1, "token-oa1"),
            manual_capabilities: IndexMap::new(),
            bounds: CatalogFetchBounds::default(),
        },
        CatalogRefreshTarget {
            identity: identity("openai_work", ProviderKind::OpenAi, &oa2, 2, false),
            models_list_url: format!("{oa2}/models"),
            credential: cred(2, "token-oa2"),
            manual_capabilities: IndexMap::new(),
            bounds: CatalogFetchBounds::default(),
        },
        CatalogRefreshTarget {
            identity: identity("openrouter", ProviderKind::OpenRouter, &or1, 3, true),
            models_list_url: format!("{or1}/models"),
            credential: cred(3, "token-or1"),
            manual_capabilities: IndexMap::new(),
            bounds: CatalogFetchBounds::default(),
        },
        CatalogRefreshTarget {
            identity: identity("openrouter_team", ProviderKind::OpenRouter, &or2, 4, false),
            models_list_url: format!("{or2}/models"),
            credential: cred(4, "token-or2"),
            manual_capabilities: IndexMap::new(),
            bounds: CatalogFetchBounds::default(),
        },
    ];

    let pub_gen = coord
        .refresh_all(targets, CancellationToken::new())
        .await
        .expect("publish");
    let snap = publisher.load();
    assert_eq!(snap.generation, pub_gen);

    let proj = snap.gated_projection();
    assert!(proj.selection_entries.contains_key("openai:shared-model"));
    assert!(
        proj.selection_entries
            .contains_key("openai_work:shared-model")
    );
    assert!(
        proj.hidden_additional_ids
            .contains(&"openai_work:shared-model".into())
    );
    assert!(snap.get("openai:shared-model").is_some());
    assert!(snap.get("openai_work:shared-model").is_none());
    assert!(proj.get_any("openai_work:shared-model").is_some());

    let reqs = rec1.lock().unwrap();
    assert!(
        reqs[0].contains("Authorization: Bearer token-oa1")
            || reqs[0].contains("authorization: Bearer token-oa1")
    );

    for h in [h1, h2, h3, h4] {
        h.join().unwrap();
    }
}

// ── 2. Pagination / bounds ─────────────────────────────────────────────────

#[tokio::test]
async fn openai_multi_page_requires_explicit_has_more_and_auth() {
    let (base, rec, handle) = spawn_scripted_server(vec![
        hop(
            "/models",
            200,
            r#"{"data":[{"id":"m1"},{"id":"m2"}],"has_more":true,"last_id":"m2"}"#,
        ),
        hop(
            "after=m2",
            200,
            r#"{"data":[{"id":"m3"}],"has_more":false}"#,
        ),
    ]);
    let id = identity("openai", ProviderKind::OpenAi, &base, 1, true);
    let result = fetch_openai_catalog(
        &format!("{base}/models"),
        "sk-test",
        &id,
        &IndexMap::new(),
        CatalogFetchBounds::default(),
        1,
        1,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let ids: Vec<_> = result
        .models
        .iter()
        .map(|m| m.upstream_model_id.as_str())
        .collect();
    assert_eq!(ids, vec!["m1", "m2", "m3"]);
    let reqs = rec.lock().unwrap();
    assert_eq!(reqs.len(), 2);
    assert!(reqs[0].contains("Bearer sk-test") || reqs[0].contains("bearer sk-test"));
    assert!(reqs[1].contains("after=m2"));
    handle.join().unwrap();
}

#[tokio::test]
async fn openai_last_id_without_has_more_is_single_page() {
    let (base, rec, handle) = spawn_scripted_server(vec![hop(
        "/models",
        200,
        r#"{"data":[{"id":"only"}],"has_more":false,"last_id":"only"}"#,
    )]);
    let id = identity("openai", ProviderKind::OpenAi, &base, 1, true);
    let result = fetch_openai_catalog(
        &format!("{base}/models"),
        "sk",
        &id,
        &IndexMap::new(),
        CatalogFetchBounds::default(),
        1,
        1,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(result.models.len(), 1);
    // Only one hop — last_id did not invent a second page.
    assert_eq!(rec.lock().unwrap().len(), 1);
    handle.join().unwrap();
}

#[tokio::test]
async fn openrouter_origin_escape_and_secret_query_rejected() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    let body = r#"{"data":[{"id":"a","supported_parameters":[]}],"links":{"next":"https://evil.example/models"}}"#;
    let handle = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = s.read(&mut buf);
        let _ = write!(
            s,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    });
    let id = identity("openrouter", ProviderKind::OpenRouter, &base, 1, true);
    let err = fetch_openrouter_catalog(
        &format!("{base}/models"),
        "sk-or",
        &id,
        &IndexMap::new(),
        CatalogFetchBounds::default(),
        1,
        1,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(err, CatalogAdapterError::OriginEscape);
    handle.join().unwrap();
}

#[tokio::test]
async fn page_bytes_bound_rejects_before_full_decode() {
    // Content-Length declares 3 MiB — must fail without needing full body.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    let handle = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = s.read(&mut buf);
        // Lie with huge Content-Length; send nothing more.
        let _ = write!(
            s,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            3 * 1024 * 1024
        );
    });
    let id = identity("openai", ProviderKind::OpenAi, &base, 1, true);
    let err = fetch_openai_catalog(
        &format!("{base}/models"),
        "sk",
        &id,
        &IndexMap::new(),
        CatalogFetchBounds::default(),
        1,
        1,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(
            err,
            CatalogAdapterError::Bound(CatalogBoundError::PageBytesExceeded { .. })
        ),
        "got {err:?}"
    );
    let _ = handle.join();
}

#[tokio::test]
async fn page_count_bound_trips_on_has_more() {
    let (base, _, handle) = spawn_scripted_server(vec![hop(
        "/models",
        200,
        r#"{"data":[{"id":"m1"}],"has_more":true,"last_id":"m1"}"#,
    )]);
    let id = identity("openai", ProviderKind::OpenAi, &base, 1, true);
    let bounds = CatalogFetchBounds {
        max_pages: 1,
        ..CatalogFetchBounds::default()
    };
    let err = fetch_openai_catalog(
        &format!("{base}/models"),
        "sk",
        &id,
        &IndexMap::new(),
        bounds,
        1,
        1,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        CatalogAdapterError::Bound(CatalogBoundError::PageCountExceeded { max: 1 })
    ));
    handle.join().unwrap();
}

#[tokio::test]
async fn model_count_bound_trips() {
    let (base, _, handle) = spawn_scripted_server(vec![hop(
        "/models",
        200,
        r#"{"data":[{"id":"a"},{"id":"b"},{"id":"c"}]}"#,
    )]);
    let id = identity("openai", ProviderKind::OpenAi, &base, 1, true);
    let bounds = CatalogFetchBounds {
        max_models: 2,
        ..CatalogFetchBounds::default()
    };
    let err = fetch_openai_catalog(
        &format!("{base}/models"),
        "sk",
        &id,
        &IndexMap::new(),
        bounds,
        1,
        1,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        CatalogAdapterError::Bound(CatalogBoundError::ModelCountExceeded { .. })
    ));
    handle.join().unwrap();
}

// ── 3. Partial failure / omit empty ────────────────────────────────────────

#[tokio::test]
async fn first_time_failure_omits_account_not_empty_lkg() {
    let home = tempfile::tempdir().unwrap();
    let publisher = Arc::new(CatalogPublisher::new());
    let coord = CatalogRefreshCoordinator::with_fixed_registry(home.path(), publisher.clone(), 1);
    let (base, _, handle) = spawn_scripted_server(vec![hop("/models", 401, r#"{"error":"no"}"#)]);
    let target = CatalogRefreshTarget {
        identity: identity("openai", ProviderKind::OpenAi, &base, 1, true),
        models_list_url: format!("{base}/models"),
        credential: cred(1, "bad"),
        manual_capabilities: IndexMap::new(),
        bounds: CatalogFetchBounds::default(),
    };
    let outcome = coord
        .refresh_all(vec![target], CancellationToken::new())
        .await;
    // May publish empty snapshot (no accounts) or None — never empty openai row.
    if let Some(pub_gen) = outcome {
        let snap = publisher.load();
        assert_eq!(snap.generation, pub_gen);
        assert_eq!(snap.gated_account_count(), 0);
    } else {
        assert_eq!(publisher.load().gated_account_count(), 0);
    }
    handle.join().unwrap();
}

#[tokio::test]
async fn partial_second_page_retains_lkg_and_siblings() {
    let home = tempfile::tempdir().unwrap();
    let publisher = Arc::new(CatalogPublisher::new());
    let coord = CatalogRefreshCoordinator::with_fixed_registry(home.path(), publisher.clone(), 1);

    let (a_base, _, a_h1) = spawn_scripted_server(vec![hop(
        "/models",
        200,
        r#"{"data":[{"id":"a1"},{"id":"a2"}]}"#,
    )]);
    let (b_base, _, b_h1) =
        spawn_scripted_server(vec![hop("/models", 200, r#"{"data":[{"id":"b1"}]}"#)]);
    let id_a = identity("openai", ProviderKind::OpenAi, &a_base, 1, true);
    let id_b = identity("openai_work", ProviderKind::OpenAi, &b_base, 2, false);

    // First refresh — store both accounts in LKG.
    let gen1 = coord
        .refresh_all(
            vec![
                CatalogRefreshTarget {
                    identity: id_a.clone(),
                    models_list_url: format!("{a_base}/models"),
                    credential: cred(1, "ta"),
                    manual_capabilities: IndexMap::new(),
                    bounds: CatalogFetchBounds::default(),
                },
                CatalogRefreshTarget {
                    identity: id_b.clone(),
                    models_list_url: format!("{b_base}/models"),
                    credential: cred(2, "tb"),
                    manual_capabilities: IndexMap::new(),
                    bounds: CatalogFetchBounds::default(),
                },
            ],
            CancellationToken::new(),
        )
        .await
        .unwrap();
    a_h1.join().unwrap();
    b_h1.join().unwrap();
    assert_eq!(publisher.load().generation, gen1);

    // Seed disk under new mock origins for failure path.
    let (a2, _, a_h2) = spawn_scripted_server(vec![
        hop(
            "/models",
            200,
            r#"{"data":[{"id":"a-new"}],"has_more":true,"last_id":"a-new"}"#,
        ),
        hop("after=", 401, r#"{"error":"nope"}"#),
    ]);
    let (b2, _, b_h2) = spawn_scripted_server(vec![hop(
        "/models",
        200,
        r#"{"data":[{"id":"b1"},{"id":"b2"}]}"#,
    )]);
    let id_a2 = identity("openai", ProviderKind::OpenAi, &a2, 1, true);
    let id_b2 = identity("openai_work", ProviderKind::OpenAi, &b2, 2, false);
    store_simple_catalog(home.path(), &id_a2, &["a1", "a2"]);
    store_simple_catalog(home.path(), &id_b2, &["b1"]);

    let coord2 = CatalogRefreshCoordinator::with_fixed_registry(home.path(), publisher.clone(), 2);
    // Seed LKG from disk.
    coord2.load_lkg_from_caches(&[id_a2.clone(), id_b2.clone()]);

    let gen2 = coord2
        .refresh_all(
            vec![
                CatalogRefreshTarget {
                    identity: id_a2,
                    models_list_url: format!("{a2}/models"),
                    credential: cred(1, "ta"),
                    manual_capabilities: IndexMap::new(),
                    bounds: CatalogFetchBounds::default(),
                },
                CatalogRefreshTarget {
                    identity: id_b2,
                    models_list_url: format!("{b2}/models"),
                    credential: cred(2, "tb"),
                    manual_capabilities: IndexMap::new(),
                    bounds: CatalogFetchBounds::default(),
                },
            ],
            CancellationToken::new(),
        )
        .await
        .unwrap();
    a_h2.join().unwrap();
    b_h2.join().unwrap();

    let snap2 = publisher.load();
    assert_eq!(snap2.generation, gen2);
    let lkg = coord2.last_known_good();
    let a_models: Vec<_> = lkg
        .get("openai")
        .unwrap()
        .models
        .iter()
        .map(|m| m.upstream_model_id.as_str())
        .collect();
    assert_eq!(a_models, vec!["a1", "a2"]);
    let b_models: Vec<_> = lkg
        .get("openai_work")
        .unwrap()
        .models
        .iter()
        .map(|m| m.upstream_model_id.as_str())
        .collect();
    assert_eq!(b_models, vec!["b1", "b2"]);
}

fn store_simple_catalog(
    home: &std::path::Path,
    identity: &CatalogAccountIdentity,
    models: &[&str],
) {
    let cache_identity = ProviderCacheIdentity::new(
        identity.instance_id.clone(),
        identity.incarnation.clone(),
        identity.kind,
        identity.api_surface,
        identity.credential_route,
        identity.endpoint_origin.clone(),
        identity.org_project_fingerprint.clone(),
        identity.credential_binding_id.clone(),
    )
    .unwrap();
    let models_json: Vec<serde_json::Value> = models
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m,
                "canonical_id": canonical_selection_id(identity, m),
            })
        })
        .collect();
    let entry = crate::provider_registry::CatalogCacheEntry {
        version: crate::provider_registry::CATALOG_CACHE_VERSION,
        provider_id: identity.instance_id.as_str().to_owned(),
        origin: crate::provider_registry::CacheOrigin::Live,
        base_url_origin: identity.endpoint_origin.clone(),
        fetched_at_unix: 1,
        models: models_json,
        baseline_version: None,
        incarnation: Some(identity.incarnation.clone()),
        provider_kind: Some(identity.kind),
        api_surface: Some(identity.api_surface),
        credential_route: Some(identity.credential_route),
        credential_binding_id: Some(identity.credential_binding_id.clone()),
        org_project_fingerprint: None,
        catalog_generation: 1,
        lifecycle_generation: None,
    };
    ProviderCacheStore::store_catalog(home, &cache_identity, &entry).unwrap();
}

// ── 4–5. Stale gen / concurrent ────────────────────────────────────────────

#[tokio::test]
async fn stale_publication_generation_discarded() {
    let publisher = CatalogPublisher::new();
    let gen1 = publisher.begin_generation();
    let gen2 = publisher.begin_generation();
    let mut accounts = IndexMap::new();
    accounts.insert(
        "openai".into(),
        InstanceCatalogResult {
            provider_instance_id: "openai".into(),
            provider_kind: ProviderKind::OpenAi,
            api_surface: ApiSurface::OpenAiPlatform,
            credential_route: CredentialRoute::ApiKey,
            endpoint_origin: "https://api.openai.com".into(),
            org_project_fingerprint: String::new(),
            incarnation: None,
            credential_binding_id: None,
            registry_generation: 1,
            catalog_generation: 1,
            publication_generation: gen1,
            source: CatalogFetchSource::Live,
            truncation: CatalogTruncationReason::Complete,
            models: vec![],
            diagnostic: None,
        },
    );
    assert!(!publisher.publish_if_current(gen1, 1, accounts));
    assert_eq!(publisher.current_generation(), gen2);
}

#[tokio::test]
async fn concurrent_sibling_refresh_atomic_snapshot() {
    let home = tempfile::tempdir().unwrap();
    let publisher = Arc::new(CatalogPublisher::new());
    let coord = CatalogRefreshCoordinator::with_fixed_registry(home.path(), publisher.clone(), 1);
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_c = hits.clone();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    let server = std::thread::spawn(move || {
        for i in 0..2 {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            hits_c.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(20));
            let body = format!(r#"{{"data":[{{"id":"m{i}"}}]}}"#);
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    let reader = publisher.clone();
    let reader_task = tokio::spawn(async move {
        let mut saw_partial = false;
        for _ in 0..40 {
            let snap = reader.load();
            if snap.gated_account_count() == 1 {
                saw_partial = true;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        saw_partial
    });
    let pub_gen = coord
        .refresh_all(
            vec![
                CatalogRefreshTarget {
                    identity: identity("openai", ProviderKind::OpenAi, &base, 1, true),
                    models_list_url: format!("{base}/models"),
                    credential: cred(1, "t1"),
                    manual_capabilities: IndexMap::new(),
                    bounds: CatalogFetchBounds::default(),
                },
                CatalogRefreshTarget {
                    identity: identity("openrouter", ProviderKind::OpenRouter, &base, 2, true),
                    models_list_url: format!("{base}/models"),
                    credential: cred(2, "t2"),
                    manual_capabilities: IndexMap::new(),
                    bounds: CatalogFetchBounds::default(),
                },
            ],
            CancellationToken::new(),
        )
        .await
        .unwrap();
    server.join().unwrap();
    assert!(!reader_task.await.unwrap());
    assert_eq!(publisher.load().generation, pub_gen);
    assert_eq!(publisher.load().gated_account_count(), 2);
}

// ── 6. Cache / credentials ─────────────────────────────────────────────────

#[test]
fn cache_preserves_kind_and_credential_debug_redacts() {
    let c = cred(1, "sk-super-secret");
    let dbg = format!("{c:?}");
    assert!(!dbg.contains("sk-super"));
    assert!(dbg.contains("<redacted>"));

    let home = tempfile::tempdir().unwrap();
    let mut manual = IndexMap::new();
    manual.insert("embeddings".into(), true);
    let id = identity(
        "openai_work",
        ProviderKind::OpenAi,
        "https://api.openai.com",
        1,
        false,
    );
    let models = parse_openai_models_body(
        br#"{"data":[{"id":"text-embedding-3-small"}]}"#,
        &id,
        &manual,
    )
    .unwrap();
    assert_eq!(models[0].capabilities.supports_embeddings, Some(true));
    store_simple_catalog(home.path(), &id, &["text-embedding-3-small"]);
    let loaded = load_cached_account(home.path(), &id, 1, 1).unwrap();
    assert_eq!(loaded.provider_kind, ProviderKind::OpenAi);
    let mut rotated = id.clone();
    rotated.credential_binding_id = bind(9);
    assert!(load_cached_account(home.path(), &rotated, 1, 1).is_none());
}

// ── 7. Gate-off ────────────────────────────────────────────────────────────

#[test]
fn gate_off_omits_additional_from_projection_and_get() {
    // Shared process lock with gate.rs — hold across clear/restore and publish.
    let _gate = multi_account_rollout_env_lock();
    let _restore = EnvRestore::capture(MULTI_ACCOUNT_ROLLOUT_ENV);
    _restore.clear();
    assert!(!multi_account_rollout_enabled());

    let publisher = CatalogPublisher::new();
    let pub_gen = publisher.begin_generation();
    let mut accounts = IndexMap::new();
    accounts.insert(
        "openai".into(),
        InstanceCatalogResult {
            provider_instance_id: "openai".into(),
            provider_kind: ProviderKind::OpenAi,
            api_surface: ApiSurface::OpenAiPlatform,
            credential_route: CredentialRoute::ApiKey,
            endpoint_origin: "https://api.openai.com".into(),
            org_project_fingerprint: String::new(),
            incarnation: None,
            credential_binding_id: None,
            registry_generation: 1,
            catalog_generation: 1,
            publication_generation: pub_gen,
            source: CatalogFetchSource::Live,
            truncation: CatalogTruncationReason::Complete,
            models: vec![DiscoveredModel {
                canonical_selection_id: "openai:gpt-4o".into(),
                upstream_model_id: "gpt-4o".into(),
                display_name: None,
                description: None,
                context_window: None,
                max_completion_tokens: None,
                capabilities: ProjectedCapabilities::default(),
                provider_instance_id: "openai".into(),
                provider_kind: ProviderKind::OpenAi,
                api_surface: ApiSurface::OpenAiPlatform,
                credential_route: CredentialRoute::ApiKey,
                endpoint_origin: "https://api.openai.com".into(),
            }],
            diagnostic: None,
        },
    );
    accounts.insert(
        "openai_work".into(),
        InstanceCatalogResult {
            provider_instance_id: "openai_work".into(),
            provider_kind: ProviderKind::OpenAi,
            api_surface: ApiSurface::OpenAiPlatform,
            credential_route: CredentialRoute::ApiKey,
            endpoint_origin: "https://api.openai.com".into(),
            org_project_fingerprint: String::new(),
            incarnation: None,
            credential_binding_id: None,
            registry_generation: 1,
            catalog_generation: 1,
            publication_generation: pub_gen,
            source: CatalogFetchSource::Live,
            truncation: CatalogTruncationReason::Complete,
            models: vec![DiscoveredModel {
                canonical_selection_id: "openai_work:gpt-4o".into(),
                upstream_model_id: "gpt-4o".into(),
                display_name: None,
                description: None,
                context_window: None,
                max_completion_tokens: None,
                capabilities: ProjectedCapabilities::default(),
                provider_instance_id: "openai_work".into(),
                provider_kind: ProviderKind::OpenAi,
                api_surface: ApiSurface::OpenAiPlatform,
                credential_route: CredentialRoute::ApiKey,
                endpoint_origin: "https://api.openai.com".into(),
            }],
            diagnostic: None,
        },
    );
    assert!(publisher.publish_if_current(pub_gen, 1, accounts));
    let snap = publisher.load();
    assert!(snap.get("openai:gpt-4o").is_some());
    assert!(snap.get("openai_work:gpt-4o").is_none());
    assert!(!snap.gated_projection().accounts.contains_key("openai_work"));
    assert!(
        !snap
            .gated_projection()
            .selection_entries
            .contains_key("openai_work:gpt-4o")
    );
}

#[test]
fn chatgpt_and_codex_are_not_builtin_compatibility_ids() {
    assert!(!is_built_in_compatibility_instance(
        "chatgpt",
        ProviderKind::OpenAi
    ));
    assert!(!is_built_in_compatibility_instance(
        "codex",
        ProviderKind::OpenAi
    ));
    assert!(is_built_in_compatibility_instance(
        "openai",
        ProviderKind::OpenAi
    ));
    let id = identity(
        "chatgpt",
        ProviderKind::OpenAi,
        "https://api.openai.com",
        1,
        false,
    );
    assert_eq!(canonical_selection_id(&id, "gpt-4o"), "chatgpt:gpt-4o");
}

// ── 8. Secrets / cancel ────────────────────────────────────────────────────

#[test]
fn errors_sanitize_token_query_and_bearer() {
    let s = CatalogAdapterError::sanitize_detail(
        "failed https://host/models?token=abc Authorization: Bearer sk-x",
    );
    assert_eq!(s, "redacted transport error");
}

#[tokio::test]
async fn auth_failure_has_no_body_or_url_secret() {
    let (base, _, handle) = spawn_scripted_server(vec![hop(
        "/models",
        401,
        r#"{"error":{"message":"Incorrect API key sk-leaked"}}"#,
    )]);
    let id = identity("openai", ProviderKind::OpenAi, &base, 1, true);
    let err = fetch_openai_catalog(
        &format!("{base}/models"),
        "sk-leaked-credential",
        &id,
        &IndexMap::new(),
        CatalogFetchBounds::default(),
        1,
        1,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    let msg = err.to_string();
    assert!(!msg.contains("sk-leaked"));
    assert!(matches!(
        err,
        CatalogAdapterError::AuthFailure { status: 401 }
    ));
    handle.join().unwrap();
}

#[tokio::test]
async fn pre_cancel_does_not_bump_or_store() {
    let home = tempfile::tempdir().unwrap();
    let publisher = Arc::new(CatalogPublisher::new());
    let before = publisher.current_generation();
    let coord = CatalogRefreshCoordinator::with_fixed_registry(home.path(), publisher.clone(), 1);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let (base, _, handle) = spawn_scripted_server(vec![]);
    let outcome = coord
        .refresh_all(
            vec![CatalogRefreshTarget {
                identity: identity("openai", ProviderKind::OpenAi, &base, 1, true),
                models_list_url: format!("{base}/models"),
                credential: cred(1, "t"),
                manual_capabilities: IndexMap::new(),
                bounds: CatalogFetchBounds::default(),
            }],
            cancel,
        )
        .await;
    assert!(outcome.is_none());
    assert_eq!(publisher.current_generation(), before);
    let _ = handle.join();
}

#[tokio::test]
async fn mid_flight_cancel_leaves_prior_snapshot() {
    let home = tempfile::tempdir().unwrap();
    let publisher = Arc::new(CatalogPublisher::new());
    let coord = CatalogRefreshCoordinator::with_fixed_registry(home.path(), publisher.clone(), 1);

    // Publish a prior complete snapshot.
    let (base1, _, h1) =
        spawn_scripted_server(vec![hop("/models", 200, r#"{"data":[{"id":"kept"}]}"#)]);
    let gen1 = coord
        .refresh_all(
            vec![CatalogRefreshTarget {
                identity: identity("openai", ProviderKind::OpenAi, &base1, 1, true),
                models_list_url: format!("{base1}/models"),
                credential: cred(1, "t"),
                manual_capabilities: IndexMap::new(),
                bounds: CatalogFetchBounds::default(),
            }],
            CancellationToken::new(),
        )
        .await
        .unwrap();
    h1.join().unwrap();
    assert_eq!(publisher.load().generation, gen1);
    assert!(publisher.load().get("openai:kept").is_some());

    // Black-hole port: accept never completes a body. Cancel + short deadline.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let base2 = format!("http://{addr}");
    // Keep listener alive but never accept (drop handle at end of scope).
    let _hold = listener;
    let cancel = CancellationToken::new();
    let cancel_c = cancel.clone();
    let id2 = identity("openai", ProviderKind::OpenAi, &base2, 1, true);
    store_simple_catalog(home.path(), &id2, &["kept"]);
    coord.load_lkg_from_caches(&[id2.clone()]);
    let short_bounds = CatalogFetchBounds::default()
        .with_request_timeout(Duration::from_millis(200))
        .with_max_duration(Duration::from_millis(300));
    let refresh = coord.refresh_all(
        vec![CatalogRefreshTarget {
            identity: id2.clone(),
            models_list_url: format!("{base2}/models"),
            credential: cred(1, "t"),
            manual_capabilities: IndexMap::new(),
            bounds: short_bounds,
        }],
        cancel.clone(),
    );
    tokio::pin!(refresh);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        cancel_c.cancel();
    });
    let outcome = tokio::time::timeout(Duration::from_secs(2), refresh)
        .await
        .expect("refresh must not hang past deadline/cancel");
    // Cancelled / timed-out refresh must not install "new".
    if let Some(pub_gen) = outcome {
        let snap = publisher.load();
        assert_eq!(snap.generation, pub_gen);
        assert!(snap.get("openai:new").is_none());
    }
    let still = load_cached_account(home.path(), &id2, 1, 0).unwrap();
    assert!(still.models.iter().any(|m| m.upstream_model_id == "kept"));
    // Prior selectable kept remains if gen1 still current, or LKG retained.
    if publisher.current_generation() == gen1 {
        assert!(publisher.load().get("openai:kept").is_some());
    }
}

#[test]
fn built_in_compatibility_ids_stable() {
    assert_eq!(
        canonical_selection_id(
            &identity(
                "openai",
                ProviderKind::OpenAi,
                "https://api.openai.com",
                1,
                true
            ),
            "gpt-4o"
        ),
        "openai:gpt-4o"
    );
}
