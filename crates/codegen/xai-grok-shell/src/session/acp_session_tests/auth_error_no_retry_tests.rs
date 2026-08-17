use super::support::*;
use super::*;
use crate::auth::{AuthManager, AuthMode, GrokAuth, GrokComConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;

/// Test refresher that returns a fresh token and records that it
/// was invoked. Used to drive the auth-arm success path.
struct AlwaysSucceedRefresher {
    called: Arc<AtomicBool>,
}
#[async_trait::async_trait]
impl crate::auth::refresh::TokenRefresher for AlwaysSucceedRefresher {
    async fn refresh(
        &self,
        _reason: crate::auth::refresh::RefreshReason,
    ) -> crate::auth::refresh::RefreshOutcome {
        self.called.store(true, Ordering::SeqCst);
        crate::auth::refresh::RefreshOutcome::Success(Box::new(GrokAuth {
            key: "refreshed-test-token".to_string(),
            auth_mode: AuthMode::Oidc,
            refresh_token: Some("rt-new".into()),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
            ..GrokAuth::test_default()
        }))
    }
}

/// `(tempdir, manager)` with an expired OIDC token loaded so
/// `unauthorized_recovery()` actually dispatches to the refresher.
/// Tempdir must outlive the manager (auth.json path).
fn auth_manager_with_refresher(
    refresher: Arc<dyn crate::auth::refresh::TokenRefresher>,
) -> (tempfile::TempDir, Arc<AuthManager>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let am = Arc::new(AuthManager::new(dir.path(), GrokComConfig::default()));
    am.hot_swap(GrokAuth {
        key: "initial-test-key".into(),
        auth_mode: AuthMode::Oidc,
        refresh_token: Some("rt".into()),
        expires_at: Some(chrono::Utc::now() - chrono::Duration::hours(1)),
        ..GrokAuth::test_default()
    });
    am.set_refresher(refresher);
    (dir, am)
}

/// Build a `InferenceErrorInfo` of kind Auth - the same shape the
/// inner `OaiCompatClient` emit surfaces after recording its own
/// attribution.
fn auth_error() -> xai_grok_inference::InferenceErrorInfo {
    xai_grok_inference::InferenceErrorInfo {
        kind: xai_grok_inference::InferenceErrorKind::Auth,
        message: "Unauthorized (401)".to_string(),
        status_code: Some(401),
        is_retryable: false,
        retry_after_secs: None,
        model_metadata: None,
        diagnostics: None,
        empty_response_context: None,
        doom_loop_triggers: None,
        doom_loop_aborted_at_chunk: None,
    }
}

/// Construct a test actor with the supplied `auth_manager` and
/// session-token credentials wired in. Wraps the actor in `Arc`
/// ready for `handle_sampling_failure`.
async fn make_actor_with_auth_manager(
    auth_manager: Option<Arc<AuthManager>>,
) -> (Arc<SessionActor>, mpsc::UnboundedReceiver<PersistenceMsg>) {
    make_actor_with_auth_and_credentials(
        auth_manager,
        xai_chat_state::AuthType::SessionToken,
        "initial-test-key".to_string(),
    )
    .await
}

/// Variant that pins the credential `auth_type`; the `auth_method_id` is
/// derived from it. Use [`make_actor_with_method_and_credentials`] to pin the
/// two independently.
async fn make_actor_with_auth_and_credentials(
    auth_manager: Option<Arc<AuthManager>>,
    auth_type: xai_chat_state::AuthType,
    api_key: String,
) -> (Arc<SessionActor>, mpsc::UnboundedReceiver<PersistenceMsg>) {
    let method_id = match auth_type {
        xai_chat_state::AuthType::SessionToken => "cached_token",
        xai_chat_state::AuthType::ApiKey => "xai.api_key",
    };
    make_actor_with_method_and_credentials(auth_manager, method_id, auth_type, api_key).await
}

/// Pin the ACP `auth_method_id` and credential `auth_type` independently. The
/// gate keys off the stable `auth_method_id`, so this reproduces the regression:
/// a session method whose `creds.auth_type` has transiently collapsed to
/// `ApiKey` (session-token cache miss + `XAI_API_KEY`).
async fn make_actor_with_method_and_credentials(
    auth_manager: Option<Arc<AuthManager>>,
    auth_method_id: &str,
    auth_type: xai_chat_state::AuthType,
    api_key: String,
) -> (Arc<SessionActor>, mpsc::UnboundedReceiver<PersistenceMsg>) {
    let (gateway_tx, _) = mpsc::unbounded_channel();
    let (persistence_tx, persistence_rx) = mpsc::unbounded_channel();
    let mut actor = create_test_actor(50_000, 100_000, 85, gateway_tx, persistence_tx).await;
    actor.auth_manager = auth_manager;
    actor.auth_method_id = test_auth_method_id(auth_method_id);
    actor
        .chat_state_handle
        .update_credentials(xai_chat_state::Credentials {
            api_key: Some(api_key),
            auth_type,
            ..Default::default()
        });
    (Arc::new(actor), persistence_rx)
}

/// `(tempdir, manager)` holding a valid OIDC token (so `get_valid_token()` is a
/// cache hit). The tempdir must outlive the manager (auth.json path).
fn auth_manager_with_valid_token(key: &str) -> (tempfile::TempDir, Arc<AuthManager>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let am = Arc::new(AuthManager::new(dir.path(), GrokComConfig::default()));
    am.hot_swap(GrokAuth {
        key: key.into(),
        auth_mode: AuthMode::Oidc,
        refresh_token: Some("rt".into()),
        expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        ..GrokAuth::test_default()
    });
    (dir, am)
}

/// Sub-case 1: no auth_manager -> falls through, no emit.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(attribution_emit_count)]
async fn no_emit_when_auth_manager_is_none() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _rx) = make_actor_with_auth_manager(None).await;
            crate::auth::attribution::reset_test_emit_count();
            let _ = actor.handle_sampling_failure(auth_error()).await;
            assert_eq!(
                crate::auth::attribution::test_emit_count(),
                0,
                "auth arm must not emit attribution when no auth_manager is wired"
            );
        })
        .await;
}

/// Sub-case 2: no AuthManager → auth recovery is skipped entirely,
/// falls through to terminal error. Covers BYOK / API-key users
/// where no OIDC refresh is possible.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(attribution_emit_count)]
async fn no_recovery_without_auth_manager() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _rx) = make_actor_with_auth_and_credentials(
                None,
                xai_chat_state::AuthType::ApiKey,
                "xai-byok-key".to_string(),
            )
            .await;
            crate::auth::attribution::reset_test_emit_count();
            let result = actor.handle_sampling_failure(auth_error()).await;
            assert!(
                result.is_err(),
                "no auth manager must fall through to terminal error"
            );
            assert_eq!(
                crate::auth::attribution::test_emit_count(),
                0,
                "auth arm must not emit attribution without auth manager"
            );
        })
        .await;
}

/// Session-based auth + working refresher → RefreshAuthAndResubmit.
#[tokio::test(flavor = "current_thread")]
async fn sampler_401_recovery_returns_refresh_and_retry() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, _rx) = make_actor_with_auth_manager(Some(am)).await;
            let result = actor.handle_sampling_failure(auth_error()).await;
            assert!(
                matches!(result, Ok(InferenceFailureRecovery::RefreshAuthAndResubmit)),
                "session-based auth with a working refresher must return RefreshAuthAndResubmit"
            );
            assert!(called.load(Ordering::SeqCst), "refresher must be invoked");
        })
        .await;
}

/// Regression: sampler 401 with API-key auth (BYOK `env_key` /
/// `XAI_API_KEY`) must NOT attempt an OIDC session-token refresh. The
/// bearer on the wire is the static API key, so refreshing the session
/// token reports success but the retry re-sends the same rejected key —
/// an invisible 401 loop that hangs the turn. Recovery is skipped and
/// the 401 surfaces as a terminal error.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(attribution_emit_count)]
async fn sampler_401_with_api_key_auth_skips_refresh_and_surfaces_error() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, _rx) = make_actor_with_auth_and_credentials(
                Some(am),
                xai_chat_state::AuthType::ApiKey,
                "xai-byok-key".to_string(),
            )
            .await;

            let result = actor.handle_sampling_failure(auth_error()).await;

            assert!(
                result.is_err(),
                "API-key 401 must surface a terminal error, not retry"
            );
            assert!(
                !called.load(Ordering::SeqCst),
                "API-key 401 must NOT trigger an OIDC session-token refresh"
            );
        })
        .await;
}

/// Per-turn pre-flight refresh must not fire when `creds.auth_type` is
/// `ApiKey` (a BYOK model): the model's own API key must not be overwritten
/// by the session JWT.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(attribution_emit_count)]
async fn pre_flight_refresh_skips_api_key_auth_type() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, _rx) = make_actor_with_auth_and_credentials(
                Some(am),
                xai_chat_state::AuthType::ApiKey,
                "byok-api-key".to_string(),
            )
            .await;
            actor.refresh_token_if_expired().await;
            assert!(
                !called.load(Ordering::SeqCst),
                "pre-flight refresh must NOT fire for ApiKey auth_type"
            );
            assert_eq!(
                actor
                    .chat_state_handle
                    .get_credentials()
                    .await
                    .api_key
                    .as_deref(),
                Some("byok-api-key"),
                "BYOK api_key must not be overwritten by session token refresh"
            );
        })
        .await;
}

/// Hard-expired session token: pre-flight must call the refresher and must
/// not leave credentials stuck while pretending the JWT/config path applies.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(attribution_emit_count)]
async fn pre_flight_refreshes_hard_expired_session_token() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            assert!(
                !am.has_usable_token(),
                "precondition: access token is hard-expired"
            );

            let (actor, _rx) = make_actor_with_auth_manager(Some(am.clone())).await;
            actor.refresh_token_if_expired().await;

            assert!(
                called.load(Ordering::SeqCst),
                "pre-flight must invoke the refresher for a hard-expired session token"
            );
            assert_eq!(
                actor
                    .chat_state_handle
                    .get_credentials()
                    .await
                    .api_key
                    .as_deref(),
                Some("refreshed-test-token"),
                "credentials must be updated to the refreshed bearer"
            );
            assert!(am.has_usable_token());
        })
        .await;
}

/// Hard-expired + failed refresh: do not fall through to JWT/config.toml;
/// leave credentials unchanged so 401 recovery remains the safety net.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(attribution_emit_count)]
async fn pre_flight_hard_expired_refresh_failure_skips_jwt_fallthrough() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> = Arc::new({
                struct AlwaysFail(Arc<std::sync::atomic::AtomicU32>);
                #[async_trait::async_trait]
                impl crate::auth::refresh::TokenRefresher for AlwaysFail {
                    async fn refresh(
                        &self,
                        _: crate::auth::refresh::RefreshReason,
                    ) -> crate::auth::refresh::RefreshOutcome {
                        self.0.fetch_add(1, Ordering::SeqCst);
                        crate::auth::refresh::RefreshOutcome::transient("refresh failed")
                    }
                }
                AlwaysFail(call_count.clone())
            });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, _rx) = make_actor_with_auth_manager(Some(am.clone())).await;

            actor.refresh_token_if_expired().await;

            assert!(
                call_count.load(Ordering::SeqCst) >= 1,
                "pre-flight must attempt refresh"
            );
            assert_eq!(
                actor
                    .chat_state_handle
                    .get_credentials()
                    .await
                    .api_key
                    .as_deref(),
                Some("initial-test-key"),
                "failed hard-expired pre-flight must not invent a JWT/config bearer"
            );
            assert!(
                !am.has_usable_token(),
                "token remains hard-expired after failed refresh"
            );
            assert!(
                am.permanent_failure().is_none(),
                "transient refresh failure must not poison permanent_failure"
            );
        })
        .await;
}

/// Proactive refresh keeps the cache hot so `refresh_token_if_expired`
/// (per-turn pre-flight) is a cache hit — the refresher fires once
/// (proactive), then the per-turn call sees the fresh token without
/// hitting the IdP again.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(attribution_emit_count)]
async fn proactive_refresh_makes_per_turn_refresh_a_cache_hit() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> = Arc::new({
                struct Counting(Arc<std::sync::atomic::AtomicU32>);
                #[async_trait::async_trait]
                impl crate::auth::refresh::TokenRefresher for Counting {
                    async fn refresh(
                        &self,
                        _: crate::auth::refresh::RefreshReason,
                    ) -> crate::auth::refresh::RefreshOutcome {
                        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        crate::auth::refresh::RefreshOutcome::Success(Box::new(GrokAuth {
                            key: "proactive-fresh".into(),
                            auth_mode: AuthMode::Oidc,
                            refresh_token: Some("rt-new".into()),
                            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
                            ..GrokAuth::test_default()
                        }))
                    }
                }
                Counting(call_count.clone())
            });

            let (_dir, am) = auth_manager_with_refresher(refresher);
            let cancel = tokio_util::sync::CancellationToken::new();
            am.start_proactive_refresh(cancel.clone());

            // Wait for proactive task to fire.
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            assert!(
                call_count.load(Ordering::SeqCst) >= 1,
                "proactive task must have fired"
            );
            let count_after_proactive = call_count.load(Ordering::SeqCst);

            // Now run refresh_token_if_expired (the per-turn pre-flight).
            // It should see the proactively-refreshed token and NOT invoke
            // the refresher again.
            let (actor, _rx) = make_actor_with_auth_manager(Some(am)).await;
            actor.refresh_token_if_expired().await;

            assert_eq!(
                call_count.load(Ordering::SeqCst),
                count_after_proactive,
                "per-turn refresh must NOT call the refresher again (cache hit)"
            );
            assert_eq!(
                actor
                    .chat_state_handle
                    .get_credentials()
                    .await
                    .api_key
                    .as_deref(),
                Some("proactive-fresh"),
                "per-turn refresh must pick up the proactively-refreshed token"
            );

            cancel.cancel();
        })
        .await;
}

fn model_not_found_error() -> xai_grok_inference::InferenceErrorInfo {
    xai_grok_inference::InferenceErrorInfo {
            kind: xai_grok_inference::InferenceErrorKind::Api,
            message: "API error (status 404 Not Found): The model grok-build does not exist or your team does not have access".into(),
            status_code: Some(404),
            is_retryable: false,
            retry_after_secs: None,
            model_metadata: None,
            diagnostics: None,
            empty_response_context: None,
            doom_loop_triggers: None,
            doom_loop_aborted_at_chunk: None,
        }
}

/// 404 model-not-found with a legacy WebLogin token appends a
/// "Legacy auth detected" hint to the error message.
#[tokio::test(flavor = "current_thread")]
async fn legacy_auth_hint_on_404_model_not_found() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().expect("tempdir");
            let am = Arc::new(AuthManager::new(dir.path(), GrokComConfig::default()));
            am.hot_swap(GrokAuth {
                key: "legacy-token".into(),
                auth_mode: AuthMode::WebLogin,
                ..GrokAuth::test_default()
            });

            let (actor, _rx) = make_actor_with_auth_manager(Some(am)).await;
            let result = actor.handle_sampling_failure(model_not_found_error()).await;
            let err = match result {
                Err(e) => e,
                Ok(_) => panic!("expected Err from handle_sampling_failure"),
            };
            let data = err.data.unwrap();
            let msg = data.as_str().unwrap();
            assert!(
                msg.contains("deprecated authentication method"),
                "404 with WebLogin must include deprecation message, got: {msg}"
            );
            assert!(
                msg.contains("/providers") || msg.contains("provider connect"),
                "hint must mention provider-scoped reconnect, got: {msg}"
            );
            assert!(
                !msg.contains("grok login") && !msg.contains("grok logout"),
                "must not mention global login/logout, got: {msg}"
            );
            assert!(
                msg.contains("Version:"),
                "must show client version, got: {msg}"
            );
        })
        .await;
}

/// Build a 401-shaped error that bypasses step 4b's auth recovery.
///
/// In production, 401s arrive as `InferenceErrorKind::Auth` with
/// `status_code: None`. Step 4b intercepts `Auth`-kind errors and
/// runs the full recovery chain — which succeeds on devbox/CI
/// environments via SA-token mint, masking the hint.
///
/// Using `Api` kind + `status_code: Some(401)` exercises the hint
/// condition (`status_code == Some(401)`) without triggering
/// recovery, making the test environment-independent.
fn unauthorized_401_error() -> xai_grok_inference::InferenceErrorInfo {
    xai_grok_inference::InferenceErrorInfo {
            kind: xai_grok_inference::InferenceErrorKind::Api,
            message: "Unauthorized (401) from https://cli-chat-proxy.grok.com/v1/responses: {\"error\":\"Invalid or expired credentials (auth_kind=bearer, x_xai_token_auth=xai-grok-cli, upstream=Unauthenticated, reason=no auth context)\"}".into(),
            status_code: Some(401),
            is_retryable: false,
            retry_after_secs: None,
            model_metadata: None,
            diagnostics: None,
            empty_response_context: None,
            doom_loop_triggers: None,
            doom_loop_aborted_at_chunk: None,
        }
}

/// 401 Unauthorized with a legacy WebLogin token appends a
/// "Legacy auth detected" hint to the error message.
#[tokio::test(flavor = "current_thread")]
async fn legacy_auth_hint_on_401_unauthorized() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().expect("tempdir");
            let am = Arc::new(AuthManager::new(dir.path(), GrokComConfig::default()));
            am.hot_swap(GrokAuth {
                key: "legacy-token".into(),
                auth_mode: AuthMode::WebLogin,
                ..GrokAuth::test_default()
            });

            let (actor, _rx) = make_actor_with_auth_manager(Some(am)).await;
            let result = actor
                .handle_sampling_failure(unauthorized_401_error())
                .await;
            let err = match result {
                Err(e) => e,
                Ok(_) => panic!("expected Err from handle_sampling_failure"),
            };
            let data = err.data.unwrap();
            let msg = data.as_str().unwrap();
            assert!(
                msg.contains("deprecated authentication method"),
                "401 with WebLogin must include deprecation message, got: {msg}"
            );
            assert!(
                msg.contains("/providers") || msg.contains("provider connect"),
                "hint must mention provider-scoped reconnect, got: {msg}"
            );
            assert!(
                !msg.contains("grok login") && !msg.contains("grok logout"),
                "must not mention global login/logout, got: {msg}"
            );
        })
        .await;
}

/// 401 with OIDC auth must NOT append the legacy WebLogin hint; terminal path
/// is structured xAI OAuth repair via `/providers` (no global `/login`).
#[tokio::test(flavor = "current_thread")]
async fn no_legacy_hint_on_401_for_oidc_auth() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().expect("tempdir");
            let am = Arc::new(AuthManager::new(dir.path(), GrokComConfig::default()));
            am.hot_swap(GrokAuth {
                key: "oidc-token".into(),
                auth_mode: AuthMode::Oidc,
                refresh_token: Some("rt".into()),
                expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
                ..GrokAuth::test_default()
            });

            let (actor, _rx) = make_actor_with_auth_manager(Some(am)).await;
            let result = actor
                .handle_sampling_failure(unauthorized_401_error())
                .await;
            let err = match result {
                Err(e) => e,
                Ok(_) => panic!("expected Err from handle_sampling_failure"),
            };
            let data = err.data.unwrap();
            let msg = data
                .get("message")
                .and_then(|v| v.as_str())
                .or_else(|| data.as_str())
                .unwrap();
            assert!(
                !msg.contains("deprecated authentication method"),
                "OIDC auth must NOT trigger WebLogin deprecation on 401, got: {msg}"
            );
            assert!(
                msg.contains("/providers") && msg.contains("xAI") && msg.contains("OAuth"),
                "OIDC 401 terminal must be structured xAI OAuth /providers repair, got: {msg}"
            );
            assert!(
                !msg.contains("/login") && !msg.contains("grok login"),
                "must not mention global login, got: {msg}"
            );
        })
        .await;
}

/// 404 model-not-found with OIDC auth must NOT append the legacy hint.
#[tokio::test(flavor = "current_thread")]
async fn no_legacy_hint_for_oidc_auth() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().expect("tempdir");
            let am = Arc::new(AuthManager::new(dir.path(), GrokComConfig::default()));
            am.hot_swap(GrokAuth {
                key: "oidc-token".into(),
                auth_mode: AuthMode::Oidc,
                refresh_token: Some("rt".into()),
                expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
                ..GrokAuth::test_default()
            });

            let (actor, _rx) = make_actor_with_auth_manager(Some(am)).await;
            let result = actor.handle_sampling_failure(model_not_found_error()).await;
            let err = match result {
                Err(e) => e,
                Ok(_) => panic!("expected Err from handle_sampling_failure"),
            };
            let data = err.data.unwrap();
            let msg = data
                .get("message")
                .and_then(|v| v.as_str())
                .or_else(|| data.as_str())
                .unwrap();
            assert!(
                !msg.contains("deprecated authentication method"),
                "OIDC auth must NOT trigger WebLogin deprecation, got: {msg}"
            );
            assert!(
                msg.contains("Auth:      Oidc"),
                "OIDC 404 must show auth mode in enriched message, got: {msg}"
            );
            assert!(
                msg.contains("Version:"),
                "OIDC 404 must show version in enriched message, got: {msg}"
            );
        })
        .await;
}

// Regression group: a live session whose `auth_type` transiently reads `ApiKey`
// must still recover, because the gate keys off the stable `auth_method_id`.
#[test]
fn session_token_auth_gate_truth_table() {
    use crate::agent::auth_method::{ModelByok, session_token_auth_gate as gate};
    // Non-session methods never refresh, regardless of BYOK status or endpoint.
    for fp in [false, true] {
        assert!(!gate(false, ModelByok::NotByok, fp));
        assert!(!gate(false, ModelByok::Byok, fp));
        assert!(!gate(false, ModelByok::Unknown, fp));
        // Session method: a definite classification ignores the endpoint —
        // NotByok always refreshes (only ever routes to the session endpoint),
        // a genuine per-model Byok never does.
        assert!(gate(true, ModelByok::NotByok, fp));
        assert!(!gate(true, ModelByok::Byok, fp));
    }
    // Session method + Unknown BYOK: refresh only against a first-party xAI
    // host, so a transiently-unclassifiable config can't demote a live session
    // (the stale-token 401 regression) yet the session token never leaks to a
    // third-party BYOK endpoint. This arm was unconditionally `false` pre-fix.
    assert!(gate(true, ModelByok::Unknown, true));
    assert!(!gate(true, ModelByok::Unknown, false));
}

/// Pre-fix, the gate read `auth_type` and skipped recovery here, 401'ing every
/// turn until restart.
#[tokio::test(flavor = "current_thread")]
async fn sampler_401_session_method_with_stale_api_key_auth_type_still_recovers() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::ApiKey,
                "stale-session-jwt".to_string(),
            )
            .await;

            let result = actor.handle_sampling_failure(auth_error()).await;

            assert!(
                matches!(result, Ok(InferenceFailureRecovery::RefreshAuthAndResubmit)),
                "session-based method must recover even when auth_type transiently reads ApiKey"
            );
            assert!(
                called.load(Ordering::SeqCst),
                "the OIDC refresher must be invoked for a session-based method"
            );
        })
        .await;
}

/// Same regression via the `oidc` method id (the other session-based variant).
#[tokio::test(flavor = "current_thread")]
async fn sampler_401_oidc_method_with_stale_api_key_auth_type_still_recovers() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "oidc",
                xai_chat_state::AuthType::ApiKey,
                "stale-session-jwt".to_string(),
            )
            .await;

            let result = actor.handle_sampling_failure(auth_error()).await;

            assert!(
                matches!(result, Ok(InferenceFailureRecovery::RefreshAuthAndResubmit)),
                "oidc method must recover even when auth_type transiently reads ApiKey"
            );
            assert!(
                called.load(Ordering::SeqCst),
                "the OIDC refresher must be invoked"
            );
        })
        .await;
}

/// Without the live bearer resolver here the sampler would sign requests with
/// the stale buffered token.
#[tokio::test(flavor = "current_thread")]
async fn reconstruct_full_config_wires_bearer_resolver_for_session_method_despite_api_key_auth_type()
 {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (_dir, am) = auth_manager_with_valid_token("fresh-session-token");
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::ApiKey,
                "stale-session-jwt".to_string(),
            )
            .await;

            let cfg = actor.reconstruct_full_config().await.expect("reconstruct");

            assert!(
                cfg.bearer_resolver.is_some(),
                "session-based method must use the live bearer resolver, not the buffered key"
            );
        })
        .await;
}

/// Negative: a genuine `xai.api_key` method keeps its configured key on the
/// wire (no live resolver).
#[tokio::test(flavor = "current_thread")]
async fn reconstruct_full_config_no_bearer_resolver_for_api_key_method() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (_dir, am) = auth_manager_with_valid_token("session-token");
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "xai.api_key",
                xai_chat_state::AuthType::ApiKey,
                "xai-static-key".to_string(),
            )
            .await;

            let cfg = actor.reconstruct_full_config().await.expect("reconstruct");

            assert!(
                cfg.bearer_resolver.is_none(),
                "api-key method must keep its configured bearer (no live resolver)"
            );
        })
        .await;
}

/// H4 wire shaping (per-turn reconstruction): `reconstruct_full_config` mirrors
/// `inference_config_for_model` — when the resolved catalog model explicitly
/// disclaims reasoning support (`Some(false)`), `reasoning_effort` is stripped
/// even if stale session state left an effort set on the chat-state config.
#[tokio::test(flavor = "current_thread")]
async fn reconstruct_full_config_strips_reasoning_effort_for_unsupported_model() {
    use xai_grok_inference_types::{ReasoningEffort, ReasoningEffortSelection};
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _rx) = make_actor_with_method_and_credentials(
                None,
                "xai.api_key",
                xai_chat_state::AuthType::ApiKey,
                "xai-static-key".to_string(),
            )
            .await;
            // Stale session state: an effort stamped onto the chat-state config.
            let mut cfg = actor.chat_state_handle.get_inference_settings().await.unwrap();
            cfg.reasoning_effort = Some(ReasoningEffort::High);
            actor.chat_state_handle.update_inference_settings(cfg);
            // Catalog model "test" (the chat-state model id) explicitly
            // disclaims reasoning support.
            let mut entry = crate::agent::config::ModelEntry {
                info: crate::agent::config::ModelInfo::fallback("test"),
                model_provider: None,
                api_key: None,
                env_key: None,
                auth_provider: None,
                api_base_url: None,
            };
            entry.info.supports_reasoning_effort = Some(false);
            entry.info.reasoning_effort_selection = ReasoningEffortSelection::Unsupported;
            entry.info.reasoning_effort = Some(ReasoningEffort::High);
            actor.models_manager.insert_test_entry("test", entry);

            let cfg = actor.reconstruct_full_config().await.expect("reconstruct");

            assert_eq!(
                cfg.reasoning_effort,
                None,
                "per-turn reconstruction must strip reasoning_effort for a model that explicitly disclaims support",
            );
        })
        .await;
}

/// H4 wire shaping (per-turn reconstruction): `None` (unknown) honors an
/// explicit effort, matching the initial-build path.
#[tokio::test(flavor = "current_thread")]
async fn reconstruct_full_config_honors_reasoning_effort_when_support_unknown() {
    use xai_grok_inference_types::ReasoningEffort;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _rx) = make_actor_with_method_and_credentials(
                None,
                "xai.api_key",
                xai_chat_state::AuthType::ApiKey,
                "xai-static-key".to_string(),
            )
            .await;
            let mut cfg = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .unwrap();
            cfg.reasoning_effort = Some(ReasoningEffort::Low);
            actor.chat_state_handle.update_inference_settings(cfg);
            // Manual TOML model: `supports_reasoning_effort = None` (unknown).
            let mut entry = crate::agent::config::ModelEntry {
                info: crate::agent::config::ModelInfo::fallback("test"),
                model_provider: None,
                api_key: None,
                env_key: None,
                auth_provider: None,
                api_base_url: None,
            };
            entry.info.supports_reasoning_effort = None;
            actor.models_manager.insert_test_entry("test", entry);

            let cfg = actor.reconstruct_full_config().await.expect("reconstruct");

            assert_eq!(
                cfg.reasoning_effort,
                Some(ReasoningEffort::Low),
                "None (unknown) must honor an explicit reasoning_effort",
            );
        })
        .await;
}

/// The pre-flight refresh heals a transiently-`ApiKey` session by writing the
/// fresh session token back into `creds.api_key`.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(attribution_emit_count)]
async fn pre_flight_refresh_heals_session_method_with_stale_api_key_auth_type() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (_dir, am) = auth_manager_with_valid_token("fresh-session-token");
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::ApiKey,
                "stale-session-jwt".to_string(),
            )
            .await;

            actor.refresh_token_if_expired().await;

            assert_eq!(
                actor
                    .chat_state_handle
                    .get_credentials()
                    .await
                    .api_key
                    .as_deref(),
                Some("fresh-session-token"),
                "session-based pre-flight refresh must heal a stale api_key with the live token"
            );
        })
        .await;
}

/// End-to-end for the frozen-gate bug: a session born on `xai.api_key` (gate
/// inactive) must adopt a later OIDC `/login` on the SAME actor -- the shared
/// `auth_method_id` handle is flipped in place (no re-spawn), so the next turn
/// wires the live bearer resolver and heals the stale key.
#[tokio::test(flavor = "current_thread")]
async fn session_born_on_api_key_recovers_after_oidc_login_without_restart() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (_dir, am) = auth_manager_with_valid_token("fresh-oidc-token");
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "xai.api_key",
                xai_chat_state::AuthType::ApiKey,
                "stale-session-jwt".to_string(),
            )
            .await;

            // Born on api_key: the gate is inactive, so no live resolver.
            assert!(
                actor
                    .reconstruct_full_config()
                    .await
                    .expect("reconstruct")
                    .bearer_resolver
                    .is_none(),
                "api-key session must not use the live resolver before login"
            );

            // Simulate the agent's `authenticate` publishing an OIDC method into
            // the shared handle this running actor already holds (no re-spawn).
            actor
                .auth_method_id
                .store(Some(std::sync::Arc::new(acp::AuthMethodId::new("oidc"))));

            // The gate is recomputed each turn from the shared handle, so the
            // flip alone activates the live resolver on the very next turn --
            // no re-spawn, before any token refresh runs.
            assert!(
                actor
                    .reconstruct_full_config()
                    .await
                    .expect("reconstruct")
                    .bearer_resolver
                    .is_some(),
                "flipping the shared handle activates the resolver on the next turn"
            );

            // The pre-flight refresh then heals the stale api_key with the live token.
            actor.refresh_token_if_expired().await;
            assert_eq!(
                actor
                    .chat_state_handle
                    .get_credentials()
                    .await
                    .api_key
                    .as_deref(),
                Some("fresh-oidc-token"),
                "the stale api_key must be healed with the fresh OIDC token"
            );
        })
        .await;
}

// Per-model BYOK memo (`SessionActor::model_auth_memo`): a definite cached
// status is served without recomputing, and the memo keys on `model_id`.

/// The cache-hit branch is what lets a later config parse failure (`Unknown`)
/// fall back to the last-known-good status.
#[tokio::test(flavor = "current_thread")]
async fn model_auth_memo_serves_cached_status_and_keys_on_model() {
    use crate::agent::auth_method::ModelByok;
    use crate::agent::config::ModelAuthFacts;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _rx) = make_actor_with_method_and_credentials(
                None,
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "k".to_string(),
            )
            .await;

            actor
                .model_auth_memo
                .replace(Some(crate::session::acp_session::ModelAuthMemo {
                    model_id: "model-a".to_string(),
                    facts: ModelAuthFacts {
                        byok: ModelByok::Byok,
                        auth_scheme: Default::default(),
                        include_message_model_id: true,
                    },
                    provider: None,
                }));

            // Cache hit: served without consulting config.
            assert_eq!(actor.model_auth_facts("model-a").byok, ModelByok::Byok);

            // Different model re-resolves rather than serving the stale `Byok`.
            assert_ne!(actor.model_auth_facts("model-b").byok, ModelByok::Byok);
        })
        .await;
}

/// A session method whose active model is a genuine per-model BYOK model keeps
/// the model's own key on the wire (no live resolver).
#[tokio::test(flavor = "current_thread")]
async fn reconstruct_full_config_no_bearer_resolver_for_byok_model_on_session_method() {
    use crate::agent::auth_method::ModelByok;
    use crate::agent::config::ModelAuthFacts;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (_dir, am) = auth_manager_with_valid_token("session-token");
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "byok-key".to_string(),
            )
            .await;

            let model = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .map(|c| c.model)
                .unwrap_or_default();
            actor
                .model_auth_memo
                .replace(Some(crate::session::acp_session::ModelAuthMemo {
                    model_id: model,
                    facts: ModelAuthFacts {
                        byok: ModelByok::Byok,
                        auth_scheme: Default::default(),
                        include_message_model_id: true,
                    },
                    provider: None,
                }));

            let cfg = actor.reconstruct_full_config().await.expect("reconstruct");

            assert!(
                cfg.bearer_resolver.is_none(),
                "a per-model BYOK model must keep its own key even on a session method"
            );
        })
        .await;
}

/// Regression: a model-switch chokepoint must invalidate
/// the memo even when `model_id` is unchanged. Otherwise a config edit that
/// turns the current model into a per-model BYOK model on a third-party
/// `base_url` keeps serving the stale `NotByok`, leaving the gate active and
/// leaking the OIDC token cross-host.
#[tokio::test(flavor = "current_thread")]
async fn set_session_model_invalidates_byok_memo_for_same_model_id() {
    use crate::agent::auth_method::ModelByok;
    use crate::agent::config::ModelAuthFacts;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _rx) = make_actor_with_method_and_credentials(
                None,
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "k".to_string(),
            )
            .await;

            let model = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .map(|c| c.model)
                .unwrap_or_default();

            actor
                .model_auth_memo
                .replace(Some(crate::session::acp_session::ModelAuthMemo {
                    model_id: model.clone(),
                    facts: ModelAuthFacts {
                        byok: ModelByok::NotByok,
                        auth_scheme: Default::default(),
                        include_message_model_id: true,
                    },
                    provider: None,
                }));

            // Switch to the same model_id, now a per-model BYOK model on a
            // third-party endpoint.
            let cfg = xai_grok_inference::InferenceConfig {
                api_key: Some("byok-key".to_string()),
                base_url: "https://third-party.example/v1".to_string(),
                model: model.clone(),
                max_completion_tokens: None,
                temperature: None,
                top_p: None,
                openrouter_fallback_models: Vec::new(),
                openrouter_provider_preferences: None,
                openrouter_plugins: Vec::new(),
                openrouter_pacing: false,
                zai_tool_stream: false,
                zai_thinking: None,
                api_backend: crate::inference::ApiBackend::ChatCompletions,
                include_message_model_id: true,
                auth_scheme: Default::default(),
                extra_headers: Default::default(),
                context_window: 256_000,
                client_version: None,
                force_http1: false,
                max_retries: None,
                stream_tool_calls: false,
                idle_timeout_secs: None,
                client_identifier: None,
                reasoning_effort: None,
                deployment_id: None,
                user_id: None,
                origin_client: None,
                attribution_callback: None,
                bearer_resolver: None,
                supports_backend_search: false,
                supports_native_schema: None,
                supports_strict_tools: None,
                supports_image_input: None,
                supports_audio_input: None,
                supports_video_input: None,
                compactions_remaining: None,
                compaction_at_tokens: None,
                doom_loop_recovery: None,
                header_injector: None,
                provider_identity: Default::default(),
            };
            let _ = actor
                .handle_set_session_model(
                    acp::ModelId::new(cfg.model.clone()),
                    cfg,
                    false,
                    false,
                    true,
                    85,
                    crate::agent::execution_backend::ExecutionBackend::NativeInference,
                )
                .await;

            assert!(
                actor.model_auth_memo.borrow().is_none(),
                "a model switch must invalidate the per-model BYOK memo so the next \
                 reconstruct recomputes under the current config"
            );
        })
        .await;
}

use crate::auth::test_counting_provider as counting_provider;

/// Seed the per-model memo so `model_auth_provider` resolves without a
/// config load.
async fn seed_provider_memo(actor: &Arc<SessionActor>, provider: crate::auth::AuthProviderRef) {
    let model = actor
        .chat_state_handle
        .get_inference_settings()
        .await
        .map(|c| c.model)
        .unwrap_or_default();
    actor
        .model_auth_memo
        .replace(Some(crate::session::acp_session::ModelAuthMemo {
            model_id: model,
            facts: crate::agent::config::ModelAuthFacts {
                byok: crate::agent::auth_method::ModelByok::Byok,
                auth_scheme: Default::default(),
                include_message_model_id: true,
            },
            provider: Some(provider),
        }));
}

/// Regression: switching from a provider-backed model to a first-party model
/// must drop the minted provider token from the chat credentials, so it can
/// never ride a later request to `api.x.ai`. Mirrors the forward direction in
/// `set_session_model_invalidates_byok_memo_for_same_model_id`.
#[tokio::test(flavor = "current_thread")]
async fn switch_to_first_party_model_drops_minted_provider_token() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let provider = counting_provider("hall-pass", dir.path());
            let token = provider.ensure_fresh_token(None).await.rotated().unwrap();
            assert_eq!(token, "tok-1");

            let (actor, _rx) =
                make_actor_with_auth_and_credentials(None, xai_chat_state::AuthType::ApiKey, token)
                    .await;
            seed_provider_memo(&actor, provider).await;

            let model = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .map(|c| c.model)
                .unwrap_or_default();

            let cfg = xai_grok_inference::InferenceConfig {
                api_key: Some("session-jwt".to_string()),
                base_url: "https://api.x.ai/v1".to_string(),
                model,
                max_completion_tokens: None,
                temperature: None,
                top_p: None,
                openrouter_fallback_models: Vec::new(),
                openrouter_provider_preferences: None,
                openrouter_plugins: Vec::new(),
                openrouter_pacing: false,
                zai_tool_stream: false,
                zai_thinking: None,
                api_backend: crate::inference::ApiBackend::ChatCompletions,
                include_message_model_id: true,
                auth_scheme: Default::default(),
                extra_headers: Default::default(),
                context_window: 256_000,
                client_version: None,
                force_http1: false,
                max_retries: None,
                stream_tool_calls: false,
                idle_timeout_secs: None,
                client_identifier: None,
                reasoning_effort: None,
                deployment_id: None,
                user_id: None,
                origin_client: None,
                attribution_callback: None,
                bearer_resolver: None,
                supports_backend_search: false,
                supports_native_schema: None,
                supports_strict_tools: None,
                supports_image_input: None,
                supports_audio_input: None,
                supports_video_input: None,
                compactions_remaining: None,
                compaction_at_tokens: None,
                doom_loop_recovery: None,
                header_injector: None,
                provider_identity: xai_grok_inference::config::ProviderIdentity::Xai,
            };
            let _ = actor
                .handle_set_session_model(
                    acp::ModelId::new(cfg.model.clone()),
                    cfg,
                    false,
                    false,
                    true,
                    85,
                    crate::agent::execution_backend::ExecutionBackend::NativeInference,
                )
                .await;

            let creds = actor.chat_state_handle.get_credentials().await;
            assert_eq!(
                creds.api_key.as_deref(),
                Some("session-jwt"),
                "switching to a first-party model must install the session credential, \
                 not the minted provider token"
            );
        })
        .await;
}

/// Arm 4c: a 401 on a provider-backed model re-mints once and resubmits.
#[tokio::test(flavor = "current_thread")]
async fn sampler_401_on_provider_model_remints_and_resubmits() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let provider = counting_provider("test-4c-recover", dir.path());
            let token = provider.ensure_fresh_token(None).await.rotated().unwrap();
            assert_eq!(token, "tok-1");

            let (actor, _rx) =
                make_actor_with_auth_and_credentials(None, xai_chat_state::AuthType::ApiKey, token)
                    .await;
            seed_provider_memo(&actor, provider).await;
            crate::auth::test_backdate_provider_mint(
                "test-4c-recover",
                std::time::Duration::from_secs(60),
            );

            let result = actor.handle_sampling_failure(auth_error()).await;
            assert!(
                matches!(result, Ok(InferenceFailureRecovery::RefreshAuthAndResubmit)),
                "provider 401 must re-mint and resubmit"
            );
            let creds = actor.chat_state_handle.get_credentials().await;
            assert_eq!(
                creds.api_key.as_deref(),
                Some("tok-2"),
                "chat-state credentials must carry the re-minted token"
            );
        })
        .await;
}

/// Arm 4c also fires for a bare 401 that did not classify as `Auth`-kind.
#[tokio::test(flavor = "current_thread")]
async fn sampler_non_auth_kind_401_on_provider_model_still_recovers() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let provider = counting_provider("test-4c-non-auth-kind", dir.path());
            let token = provider.ensure_fresh_token(None).await.rotated().unwrap();

            let (actor, _rx) =
                make_actor_with_auth_and_credentials(None, xai_chat_state::AuthType::ApiKey, token)
                    .await;
            seed_provider_memo(&actor, provider).await;
            crate::auth::test_backdate_provider_mint(
                "test-4c-non-auth-kind",
                std::time::Duration::from_secs(60),
            );

            let mut error = auth_error();
            error.kind = xai_grok_inference::InferenceErrorKind::Api;
            let result = actor.handle_sampling_failure(error).await;
            assert!(
                matches!(result, Ok(InferenceFailureRecovery::RefreshAuthAndResubmit)),
                "a non-Auth-kind 401 on a provider model must still recover via 4c"
            );
            let creds = actor.chat_state_handle.get_credentials().await;
            assert_eq!(creds.api_key.as_deref(), Some("tok-2"));
        })
        .await;
}

/// A 401 on a request that went out with no key mints instead of
/// recovering.
#[tokio::test(flavor = "current_thread")]
async fn sampler_401_with_no_key_on_provider_model_mints_and_resubmits() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let provider = counting_provider("test-4c-no-key", dir.path());

            let (actor, _rx) = make_actor_with_auth_and_credentials(
                None,
                xai_chat_state::AuthType::ApiKey,
                "placeholder".to_string(),
            )
            .await;
            let mut creds = actor.chat_state_handle.get_credentials().await;
            creds.api_key = None;
            actor.chat_state_handle.update_credentials(creds);
            seed_provider_memo(&actor, provider).await;

            let result = actor.handle_sampling_failure(auth_error()).await;
            assert!(
                matches!(result, Ok(InferenceFailureRecovery::RefreshAuthAndResubmit)),
                "an unauthenticated 401 on a provider model must mint and resubmit"
            );
            let creds = actor.chat_state_handle.get_credentials().await;
            assert_eq!(creds.api_key.as_deref(), Some("tok-1"));
        })
        .await;
}

/// A provider model's 401 goes through the provider, never the session
/// refresher (4a/4b vs 4c exclusivity). The actor uses a session-based method,
/// so the gate would be active for a non-BYOK model; the BYOK memo is what
/// shadows it, which is the invariant under test.
#[tokio::test(flavor = "current_thread")]
async fn sampler_401_on_provider_model_never_refreshes_session() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let provider = counting_provider("test-4c-exclusive", dir.path());
            let token = provider.ensure_fresh_token(None).await.rotated().unwrap();

            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                token,
            )
            .await;
            seed_provider_memo(&actor, provider).await;
            crate::auth::test_backdate_provider_mint(
                "test-4c-exclusive",
                std::time::Duration::from_secs(60),
            );

            let result = actor.handle_sampling_failure(auth_error()).await;
            assert!(
                matches!(result, Ok(InferenceFailureRecovery::RefreshAuthAndResubmit)),
                "the provider arm must recover"
            );
            assert!(
                !called.load(Ordering::SeqCst),
                "session refresh must never fire for a provider-backed model"
            );
            let creds = actor.chat_state_handle.get_credentials().await;
            assert_eq!(creds.api_key.as_deref(), Some("tok-2"));
        })
        .await;
}

/// The pre-turn mirror of the exclusivity test: a cold cache mints the
/// provider token into chat-state, and the session refresher never fires. The
/// actor uses a session-based method, so the gate would be active for a
/// non-BYOK model; the BYOK memo is what keeps the refresher silent.
#[tokio::test(flavor = "current_thread")]
async fn pre_turn_on_provider_model_never_installs_session_token() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let provider = counting_provider("test-preturn-exclusive", dir.path());

            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "placeholder".to_string(),
            )
            .await;
            // Cold cache: no key on the wire yet.
            let mut creds = actor.chat_state_handle.get_credentials().await;
            creds.api_key = None;
            actor.chat_state_handle.update_credentials(creds);
            seed_provider_memo(&actor, provider).await;

            actor.refresh_token_if_expired().await;

            let creds = actor.chat_state_handle.get_credentials().await;
            assert_eq!(
                creds.api_key.as_deref(),
                Some("tok-1"),
                "the cold pre-turn hook must mint the provider token"
            );
            assert!(
                !called.load(Ordering::SeqCst),
                "the session refresher must never fire for a provider-backed model"
            );
        })
        .await;
}

/// A token rejected moments after mint surfaces the 401 (fresh-mint
/// guard).
#[tokio::test(flavor = "current_thread")]
async fn sampler_401_on_fresh_provider_token_surfaces_error() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let provider = counting_provider("test-4c-guard", dir.path());
            let token = provider.ensure_fresh_token(None).await.rotated().unwrap();

            let (actor, _rx) = make_actor_with_auth_and_credentials(
                None,
                xai_chat_state::AuthType::ApiKey,
                token.clone(),
            )
            .await;
            seed_provider_memo(&actor, provider).await;

            let result = actor.handle_sampling_failure(auth_error()).await;
            assert!(
                result.is_err(),
                "a fresh-minted rejected token must surface the 401, not loop"
            );
            let creds = actor.chat_state_handle.get_credentials().await;
            assert_eq!(
                creds.api_key.as_deref(),
                Some(token.as_str()),
                "credentials must be unchanged when the guard blocks the re-mint"
            );
        })
        .await;
}

/// Milestone 1 regression: a Moonshot model routed through OpenRouter that
/// receives HTTP 401 must retain OpenRouter identity on the shell→pager
/// RetryState boundary, never trigger xAI OAuth recovery, and never emit a
/// global `/login` repair path.
#[tokio::test(flavor = "current_thread")]
async fn moonshot_openrouter_401_retains_openrouter_provider_context() {
    use crate::agent::config::{ModelEntry, ModelInfo};
    use crate::agent::model_providers::{ModelProviderKind, ResolvedModelProvider};
    use crate::extensions::notification::SessionUpdate as XaiSessionUpdate;
    use crate::session::storage::SessionUpdate;
    use xai_grok_inference_types::ApiErrorDiagnostics;

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, mut persistence_rx) = make_actor_with_auth_and_credentials(
                Some(am),
                xai_chat_state::AuthType::ApiKey,
                "or-bad-key".to_string(),
            )
            .await;

            // Catalog: Moonshot model served exclusively through OpenRouter.
            let catalog_id = "openrouter:moonshotai/kimi-k2";
            let model_slug = "moonshotai/kimi-k2";
            let mut entry = ModelEntry {
                info: ModelInfo::fallback(model_slug),
                model_provider: Some(ResolvedModelProvider {
                    id: "openrouter".to_string(),
                    kind: ModelProviderKind::OpenRouter,
                    openrouter_fallback_models: Vec::new(),
                    openrouter_provider_preferences: None,
                    openrouter_plugins: Vec::new(),
                    openrouter_pacing: false,
                    command: Vec::new(),
                }),
                api_key: None,
                env_key: None,
                auth_provider: None,
                api_base_url: Some("https://openrouter.ai/api/v1".to_string()),
            };
            entry.info.base_url = "https://openrouter.ai/api/v1".to_string();
            entry.info.model = model_slug.to_string();
            actor
                .models_manager
                .insert_test_entry(catalog_id, entry.clone());
            // Also index by the raw upstream slug used on the wire.
            actor.models_manager.insert_test_entry(model_slug, entry);

            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .expect("settings");
            settings.model = model_slug.to_string();
            settings.base_url = "https://openrouter.ai/api/v1".to_string();
            settings.api_backend = crate::inference::ApiBackend::ChatCompletions;
            actor.chat_state_handle.update_inference_settings(settings);

            let mut err = auth_error();
            err.message = "Unauthorized (401) from https://openrouter.ai/api/v1/chat/completions: \
                 User not found."
                .to_string();
            err.diagnostics = Some(ApiErrorDiagnostics {
                provider_name: Some("OpenRouter".to_string()),
                generation_id: Some("gen-test-moonshot-401".to_string()),
                ..Default::default()
            });

            let result = actor.handle_sampling_failure(err).await;
            assert!(
                result.is_err(),
                "OpenRouter API-key 401 must be terminal (no silent retry)"
            );
            assert!(
                !called.load(Ordering::SeqCst),
                "OpenRouter 401 must never initiate xAI OAuth / session refresh"
            );

            let mut saw_openrouter_failed = false;
            while let Ok(msg) = persistence_rx.try_recv() {
                if let PersistenceMsg::Update(SessionUpdate::Xai(notif)) = msg
                    && let XaiSessionUpdate::RetryState(
                        crate::extensions::notification::RetryState::Failed {
                            error_type,
                            message,
                            provider,
                        },
                    ) = &notif.update
                {
                    use crate::extensions::notification::PROVIDER_CREDENTIAL_ERROR_TYPE;
                    assert_eq!(error_type, PROVIDER_CREDENTIAL_ERROR_TYPE);
                    assert!(
                        message.contains("OpenRouter") && message.contains("/providers"),
                        "message={message}"
                    );
                    assert!(
                        !message.contains("/login"),
                        "message must not mention /login: {message}"
                    );
                    assert!(
                        !message.contains("Unauthorized (401)"),
                        "legacy-safe message must omit Unauthorized (401): {message}"
                    );
                    assert!(
                        !message.contains("grok login"),
                        "must not mention grok login: {message}"
                    );
                    let provider = provider
                        .as_ref()
                        .expect("OpenRouter 401 must attach provider credential failure context");
                    assert_eq!(provider.provider_id, "openrouter");
                    assert_eq!(provider.provider_name, "OpenRouter");
                    assert_eq!(
                        provider.failed_model_id.as_deref(),
                        Some(model_slug),
                        "failed model id must retain the Moonshot slug"
                    );
                    assert_eq!(provider.http_status, Some(401));
                    assert_eq!(
                        provider.generation_id.as_deref(),
                        Some("gen-test-moonshot-401")
                    );
                    assert_eq!(
                        provider.error_category.as_deref(),
                        Some(PROVIDER_CREDENTIAL_ERROR_TYPE)
                    );
                    assert!(
                        provider
                            .backend
                            .as_deref()
                            .is_some_and(|b| b.contains("chat")),
                        "backend={:?}",
                        provider.backend
                    );
                    saw_openrouter_failed = true;
                }
            }
            assert!(
                saw_openrouter_failed,
                "expected RetryState::Failed with OpenRouter provider context"
            );
        })
        .await;
}

/// Realistic regression: session-based ACP method (`cached_token`) + loaded
/// xAI WebLogin/OIDC AuthManager + OpenRouter catalog entry **without** an
/// inline model key + OpenRouter 401.
///
/// Must never call the xAI refresher, never return RefreshAuthAndResubmit,
/// and emit provider-scoped OpenRouter repair (no `/login` / `grok login`).
#[tokio::test(flavor = "current_thread")]
async fn moonshot_openrouter_401_with_weblogin_loaded_skips_xai_recovery() {
    use crate::agent::config::{ModelEntry, ModelInfo};
    use crate::agent::model_providers::{ModelProviderKind, ResolvedModelProvider};
    use crate::auth::{AuthMode, GrokAuth};
    use crate::extensions::notification::{
        PROVIDER_CREDENTIAL_ERROR_TYPE, SessionUpdate as XaiSessionUpdate,
    };
    use crate::session::storage::SessionUpdate;

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let dir = tempfile::tempdir().expect("tempdir");
            let am = Arc::new(AuthManager::new(dir.path(), GrokComConfig::default()));
            // Active WebLogin/OIDC session present concurrently.
            am.hot_swap(GrokAuth {
                key: "xai-weblogin-token".into(),
                auth_mode: AuthMode::WebLogin,
                refresh_token: Some("rt".into()),
                expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
                ..GrokAuth::test_default()
            });
            am.set_refresher(refresher);

            // Session-based ACP method (the real mis-recovery path).
            let (actor, mut persistence_rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "xai-session-jwt".to_string(),
            )
            .await;

            let model_slug = "moonshotai/kimi-k2";
            // No inline api_key — key would live in OpenRouter vault.
            let mut entry = ModelEntry {
                info: ModelInfo::fallback(model_slug),
                model_provider: Some(ResolvedModelProvider {
                    id: "openrouter".to_string(),
                    kind: ModelProviderKind::OpenRouter,
                    openrouter_fallback_models: Vec::new(),
                    openrouter_provider_preferences: None,
                    openrouter_plugins: Vec::new(),
                    openrouter_pacing: false,
                    command: Vec::new(),
                }),
                api_key: None,
                env_key: None,
                auth_provider: None,
                api_base_url: Some("https://openrouter.ai/api/v1".to_string()),
            };
            entry.info.base_url = "https://openrouter.ai/api/v1".to_string();
            entry.info.model = model_slug.to_string();
            assert!(
                !entry.has_own_credentials(),
                "fixture must have no inline key (vault-style)"
            );
            assert!(entry.is_provider_scoped_byok());
            actor.models_manager.insert_test_entry(model_slug, entry);

            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .expect("settings");
            settings.model = model_slug.to_string();
            settings.base_url = "https://openrouter.ai/api/v1".to_string();
            actor.chat_state_handle.update_inference_settings(settings);

            // Provider-scoped BYOK must disable session-token recovery.
            assert!(
                !crate::agent::auth_method::session_token_auth_gate(
                    true, // session-based ACP method
                    crate::agent::auth_method::ModelByok::Byok,
                    false,
                ),
                "Byok models must not activate session-token recovery"
            );

            let result = actor.handle_sampling_failure(auth_error()).await;
            assert!(
                result.is_err(),
                "OpenRouter 401 must be terminal, not recovery"
            );
            assert!(
                !matches!(result, Ok(InferenceFailureRecovery::RefreshAuthAndResubmit)),
                "must not return RefreshAuthAndResubmit for OpenRouter"
            );
            assert!(
                !called.load(Ordering::SeqCst),
                "must not call xAI OIDC refresher for OpenRouter failure"
            );

            let mut saw = false;
            while let Ok(msg) = persistence_rx.try_recv() {
                if let PersistenceMsg::Update(SessionUpdate::Xai(notif)) = msg
                    && let XaiSessionUpdate::RetryState(
                        crate::extensions::notification::RetryState::Failed {
                            error_type,
                            message,
                            provider,
                        },
                    ) = &notif.update
                {
                    assert_eq!(error_type, PROVIDER_CREDENTIAL_ERROR_TYPE);
                    assert!(!message.contains("/login"));
                    assert!(!message.contains("grok login"));
                    assert!(!message.contains("WebLogin"));
                    let p = provider.as_ref().expect("provider context");
                    assert_eq!(p.provider_id, "openrouter");
                    saw = true;
                }
            }
            assert!(saw, "expected OpenRouter provider-scoped failure");
        })
        .await;
}

/// Official OpenAI API-key route (no inline key) under session ACP method
/// + loaded xAI OIDC AuthManager must skip xAI recovery on 401.
#[tokio::test(flavor = "current_thread")]
async fn openai_api_key_401_with_session_method_skips_xai_recovery() {
    use crate::agent::config::{ModelEntry, ModelInfo};
    use crate::agent::model_providers::{ModelProviderKind, ResolvedModelProvider};
    use crate::auth::{AuthMode, GrokAuth};
    use crate::extensions::notification::{
        PROVIDER_CREDENTIAL_ERROR_TYPE, SessionUpdate as XaiSessionUpdate,
    };
    use crate::session::storage::SessionUpdate;

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let dir = tempfile::tempdir().expect("tempdir");
            let am = Arc::new(AuthManager::new(dir.path(), GrokComConfig::default()));
            am.hot_swap(GrokAuth {
                key: "xai-oidc-token".into(),
                auth_mode: AuthMode::Oidc,
                refresh_token: Some("rt".into()),
                expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
                ..GrokAuth::test_default()
            });
            am.set_refresher(refresher);
            let (actor, mut persistence_rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "xai-session-jwt".to_string(),
            )
            .await;

            let model_slug = "gpt-4o";
            // No inline api_key — key lives in OpenAI provider vault.
            let mut entry = ModelEntry {
                info: ModelInfo::fallback(model_slug),
                model_provider: Some(ResolvedModelProvider {
                    id: "openai".to_string(),
                    kind: ModelProviderKind::OpenAi,
                    openrouter_fallback_models: Vec::new(),
                    openrouter_provider_preferences: None,
                    openrouter_plugins: Vec::new(),
                    openrouter_pacing: false,
                    command: Vec::new(),
                }),
                api_key: None,
                env_key: None,
                auth_provider: None,
                api_base_url: Some("https://api.openai.com/v1".to_string()),
            };
            entry.info.base_url = "https://api.openai.com/v1".to_string();
            entry.info.model = model_slug.to_string();
            assert!(!entry.has_own_credentials());
            assert!(entry.is_provider_scoped_byok());
            actor.models_manager.insert_test_entry(model_slug, entry);

            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .unwrap();
            settings.model = model_slug.to_string();
            settings.base_url = "https://api.openai.com/v1".to_string();
            actor.chat_state_handle.update_inference_settings(settings);

            let result = actor.handle_sampling_failure(auth_error()).await;
            assert!(
                result.is_err(),
                "OpenAI API-key 401 must be terminal, not recovery"
            );
            assert!(!called.load(Ordering::SeqCst));
            assert!(!matches!(
                result,
                Ok(InferenceFailureRecovery::RefreshAuthAndResubmit)
            ));

            let mut saw = false;
            while let Ok(msg) = persistence_rx.try_recv() {
                if let PersistenceMsg::Update(SessionUpdate::Xai(notif)) = msg
                    && let XaiSessionUpdate::RetryState(
                        crate::extensions::notification::RetryState::Failed {
                            error_type,
                            message,
                            provider,
                        },
                    ) = &notif.update
                {
                    assert_eq!(error_type, PROVIDER_CREDENTIAL_ERROR_TYPE);
                    assert!(message.contains("OpenAI"));
                    assert!(!message.contains("/login"));
                    assert!(!message.contains("grok login"));
                    assert_eq!(
                        provider.as_ref().map(|p| p.provider_id.as_str()),
                        Some("openai")
                    );
                    saw = true;
                }
            }
            assert!(saw);
        })
        .await;
}

/// Control: first-party xAI under session method still runs OIDC recovery.
#[tokio::test(flavor = "current_thread")]
async fn first_party_xai_401_still_runs_session_recovery() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "initial-test-key".to_string(),
            )
            .await;
            // Default test model is first-party (no OpenRouter provider).
            let result = actor.handle_sampling_failure(auth_error()).await;
            assert!(
                matches!(result, Ok(InferenceFailureRecovery::RefreshAuthAndResubmit)),
                "first-party xAI must still recover via session refresh"
            );
            assert!(
                called.load(Ordering::SeqCst),
                "first-party xAI must invoke the OIDC refresher"
            );
        })
        .await;
}

/// Diagnostics allowlist canary: fake secrets in messages/keys must never
/// appear in the structured terminal-failure payload.
#[tokio::test(flavor = "current_thread")]
async fn terminal_failure_diagnostics_redact_secrets_and_prompts() {
    use crate::agent::config::{ModelEntry, ModelInfo};
    use crate::agent::model_providers::{ModelProviderKind, ResolvedModelProvider};
    use xai_grok_inference_types::ApiErrorDiagnostics;

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _rx) = make_actor_with_auth_and_credentials(
                None,
                xai_chat_state::AuthType::ApiKey,
                "sk-CANARY-OPENROUTER-SECRET-KEY".to_string(),
            )
            .await;
            let model_slug = "moonshotai/kimi-k2";
            let mut entry = ModelEntry {
                info: ModelInfo::fallback(model_slug),
                model_provider: Some(ResolvedModelProvider {
                    id: "openrouter".to_string(),
                    kind: ModelProviderKind::OpenRouter,
                    openrouter_fallback_models: Vec::new(),
                    openrouter_provider_preferences: None,
                    openrouter_plugins: Vec::new(),
                    openrouter_pacing: false,
                    command: Vec::new(),
                }),
                api_key: None,
                env_key: None,
                auth_provider: None,
                api_base_url: Some("https://openrouter.ai/api/v1".to_string()),
            };
            entry.info.base_url = "https://openrouter.ai/api/v1".to_string();
            entry.info.model = model_slug.to_string();
            actor.models_manager.insert_test_entry(model_slug, entry);
            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .unwrap();
            settings.model = model_slug.to_string();
            settings.base_url = "https://openrouter.ai/api/v1".to_string();
            actor.chat_state_handle.update_inference_settings(settings);

            let mut err = auth_error();
            err.message =
                "Unauthorized (401) Authorization: Bearer sk-CANARY-OPENROUTER-SECRET-KEY \
                 prompt=Please rewrite my confidential memo RESPONSE_BODY_CANARY"
                    .to_string();
            err.diagnostics = Some(ApiErrorDiagnostics {
                generation_id: Some("gen-safe-id".into()),
                ..Default::default()
            });

            // Exercise the path; redaction is verified by inspecting the
            // provider context construction (allowlisted fields only).
            let provider = actor
                .provider_credential_failure_context(
                    model_slug,
                    Some(401),
                    "provider_credential",
                    err.diagnostics.as_ref(),
                )
                .await
                .expect("openrouter context");
            let encoded = serde_json::to_string(&provider).expect("json");
            assert!(!encoded.contains("sk-CANARY"));
            assert!(!encoded.contains("Authorization"));
            assert!(!encoded.contains("confidential memo"));
            assert!(!encoded.contains("RESPONSE_BODY_CANARY"));
            assert!(!encoded.contains("Bearer"));
            assert_eq!(provider.generation_id.as_deref(), Some("gen-safe-id"));
            assert_eq!(provider.provider_id, "openrouter");
            let _ = actor.handle_sampling_failure(err).await;
        })
        .await;
}

/// Exact-host matching rejects URL spoof shapes.
#[tokio::test(flavor = "current_thread")]
async fn provider_host_fallback_rejects_url_spoofs() {
    use crate::extensions::notification::{approved_provider_for_exact_host, host_from_base_url};

    let cases = [
        "https://openrouter.ai.evil.invalid/api/v1",
        "https://evil.example/?next=openrouter.ai",
        "https://not-openrouter.ai/api/v1",
        "ftp://openrouter.ai/api/v1",
        "not-a-url",
    ];
    for url in cases {
        let host = host_from_base_url(url);
        if let Some(h) = host {
            assert!(
                approved_provider_for_exact_host(&h).is_none(),
                "spoof must not approve host for {url}: {h}"
            );
        }
    }
    assert_eq!(
        approved_provider_for_exact_host(
            &host_from_base_url("https://openrouter.ai/api/v1").unwrap()
        ),
        Some(("openrouter", "OpenRouter"))
    );
    assert_eq!(
        approved_provider_for_exact_host(&host_from_base_url("https://api.openai.com/v1").unwrap()),
        Some(("openai", "OpenAI"))
    );
}

// ── B1: reconstruct_full_config must not wire xAI bearer_resolver for
// provider-scoped vault models (catalog-only OpenRouter/OpenAI). ──────────

fn insert_openrouter_vault_model(actor: &SessionActor, model_slug: &str) {
    use crate::agent::config::{ModelEntry, ModelInfo};
    use crate::agent::model_providers::{ModelProviderKind, ResolvedModelProvider};
    let mut entry = ModelEntry {
        info: ModelInfo::fallback(model_slug),
        model_provider: Some(ResolvedModelProvider {
            id: "openrouter".to_string(),
            kind: ModelProviderKind::OpenRouter,
            openrouter_fallback_models: Vec::new(),
            openrouter_provider_preferences: None,
            openrouter_plugins: Vec::new(),
            openrouter_pacing: false,
            command: Vec::new(),
        }),
        api_key: None,
        env_key: None,
        auth_provider: None,
        api_base_url: Some("https://openrouter.ai/api/v1".to_string()),
    };
    entry.info.base_url = "https://openrouter.ai/api/v1".to_string();
    entry.info.model = model_slug.to_string();
    assert!(!entry.has_own_credentials());
    assert!(entry.is_provider_scoped_byok());
    actor.models_manager.insert_test_entry(model_slug, entry);
}

fn insert_openai_api_vault_model(actor: &SessionActor, model_slug: &str) {
    use crate::agent::config::{ModelEntry, ModelInfo};
    use crate::agent::model_providers::{ModelProviderKind, ResolvedModelProvider};
    let mut entry = ModelEntry {
        info: ModelInfo::fallback(model_slug),
        model_provider: Some(ResolvedModelProvider {
            id: "openai".to_string(),
            kind: ModelProviderKind::OpenAi,
            openrouter_fallback_models: Vec::new(),
            openrouter_provider_preferences: None,
            openrouter_plugins: Vec::new(),
            openrouter_pacing: false,
            command: Vec::new(),
        }),
        api_key: None,
        env_key: None,
        auth_provider: None,
        api_base_url: Some("https://api.openai.com/v1".to_string()),
    };
    entry.info.base_url = "https://api.openai.com/v1".to_string();
    entry.info.model = model_slug.to_string();
    assert!(entry.is_provider_scoped_byok());
    actor.models_manager.insert_test_entry(model_slug, entry);
}

/// Session ACP + loaded xAI OIDC + OpenRouter catalog vault model (no inline
/// key): reconstructed config must not install xAI bearer_resolver, and the
/// provider key on credentials must remain selected.
#[tokio::test(flavor = "current_thread")]
async fn reconstruct_openrouter_vault_model_no_xai_bearer_resolver() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (_dir, am) = auth_manager_with_valid_token("xai-session-jwt-for-resolver");
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "openrouter-provider-key".to_string(),
            )
            .await;

            let model_slug = "moonshotai/kimi-k2";
            insert_openrouter_vault_model(&actor, model_slug);
            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .expect("settings");
            settings.model = model_slug.to_string();
            settings.base_url = "https://openrouter.ai/api/v1".to_string();
            actor.chat_state_handle.update_inference_settings(settings);

            let cfg = actor.reconstruct_full_config().await.expect("reconstruct");
            assert!(
                cfg.bearer_resolver.is_none(),
                "OpenRouter vault model must not install xAI bearer_resolver"
            );
            assert_eq!(
                cfg.api_key.as_deref(),
                Some("openrouter-provider-key"),
                "provider key must remain selected (not overwritten by xAI session)"
            );
            assert_eq!(
                cfg.provider_identity,
                xai_grok_inference::config::ProviderIdentity::OpenRouter
            );
            // Guard: even if a resolver leaked in, it must not surface the xAI token.
            if let Some(resolver) = cfg.bearer_resolver.as_ref() {
                let bearer = resolver.current_bearer();
                assert_ne!(
                    bearer.as_deref(),
                    Some("xai-session-jwt-for-resolver"),
                    "xAI session token must never be the live bearer for OpenRouter"
                );
            }
        })
        .await;
}

/// Official OpenAI API-key vault model under session ACP: no xAI resolver.
#[tokio::test(flavor = "current_thread")]
async fn reconstruct_openai_api_vault_model_no_xai_bearer_resolver() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (_dir, am) = auth_manager_with_valid_token("xai-session-jwt-for-resolver");
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "openai-api-key-on-wire".to_string(),
            )
            .await;

            let model_slug = "gpt-4o";
            insert_openai_api_vault_model(&actor, model_slug);
            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .unwrap();
            settings.model = model_slug.to_string();
            settings.base_url = "https://api.openai.com/v1".to_string();
            actor.chat_state_handle.update_inference_settings(settings);

            let cfg = actor.reconstruct_full_config().await.expect("reconstruct");
            assert!(cfg.bearer_resolver.is_none());
            assert_eq!(cfg.api_key.as_deref(), Some("openai-api-key-on-wire"));
            assert_eq!(
                cfg.provider_identity,
                xai_grok_inference::config::ProviderIdentity::OpenAi
            );
        })
        .await;
}

/// Catalog miss + exact OpenRouter host still withholds xAI resolver.
#[tokio::test(flavor = "current_thread")]
async fn reconstruct_catalog_miss_openrouter_host_no_xai_bearer_resolver() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (_dir, am) = auth_manager_with_valid_token("xai-session-jwt-for-resolver");
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "or-key".to_string(),
            )
            .await;
            // No catalog entry — only host identity.
            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .unwrap();
            settings.model = "unknown-or-model".to_string();
            settings.base_url = "https://openrouter.ai/api/v1".to_string();
            actor.chat_state_handle.update_inference_settings(settings);

            let cfg = actor.reconstruct_full_config().await.expect("reconstruct");
            assert!(
                cfg.bearer_resolver.is_none(),
                "catalog miss on openrouter.ai must not install xAI bearer_resolver"
            );
            assert_eq!(
                cfg.provider_identity,
                xai_grok_inference::config::ProviderIdentity::OpenRouter
            );
        })
        .await;
}

/// Catalog miss + ChatGPT Codex base URL is OpenAI identity, never xAI recovery.
#[tokio::test(flavor = "current_thread")]
async fn reconstruct_catalog_miss_codex_url_is_openai_not_xai() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (_dir, am) = auth_manager_with_valid_token("xai-session-jwt");
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "chatgpt-access".to_string(),
            )
            .await;
            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .unwrap();
            settings.model = "codex-unknown".to_string();
            settings.base_url = crate::auth::chatgpt_oauth::CODEX_RESPONSES_BASE_URL.to_string();
            actor.chat_state_handle.update_inference_settings(settings);

            let cfg = actor.reconstruct_full_config().await.expect("reconstruct");
            assert!(cfg.bearer_resolver.is_none());
            assert_eq!(
                cfg.provider_identity,
                xai_grok_inference::config::ProviderIdentity::OpenAi
            );
        })
        .await;
}

/// Control: first-party xAI under session method still wires the live resolver.
#[tokio::test(flavor = "current_thread")]
async fn reconstruct_first_party_xai_still_wires_bearer_resolver() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (_dir, am) = auth_manager_with_valid_token("fresh-xai-session");
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "stale-buffered".to_string(),
            )
            .await;
            // Default test model is first-party xAI.
            let cfg = actor.reconstruct_full_config().await.expect("reconstruct");
            assert!(
                cfg.bearer_resolver.is_some(),
                "first-party xAI session method must wire live bearer_resolver"
            );
            assert_eq!(
                cfg.bearer_resolver
                    .as_ref()
                    .and_then(|r| r.current_bearer())
                    .as_deref(),
                Some("fresh-xai-session"),
            );
        })
        .await;
}

// ── B2: ChatGPT OAuth pre-turn must be scoped to the active Codex route. ─

#[test]
fn chatgpt_oauth_preturn_applies_only_to_codex_base_url() {
    use super::inference_turn::{chatgpt_oauth_preturn_applies, is_chatgpt_codex_base_url};

    assert!(is_chatgpt_codex_base_url(
        crate::auth::chatgpt_oauth::CODEX_RESPONSES_BASE_URL
    ));
    assert!(is_chatgpt_codex_base_url(
        "https://chatgpt.com/backend-api/codex/responses"
    ));
    assert!(chatgpt_oauth_preturn_applies(
        crate::auth::chatgpt_oauth::CODEX_RESPONSES_BASE_URL
    ));

    // Must never apply to unrelated routes even when ChatGPT is "connected".
    assert!(!chatgpt_oauth_preturn_applies(
        "https://openrouter.ai/api/v1"
    ));
    assert!(!chatgpt_oauth_preturn_applies("https://api.openai.com/v1"));
    assert!(!chatgpt_oauth_preturn_applies("https://api.x.ai/v1"));
    assert!(!chatgpt_oauth_preturn_applies("http://127.0.0.1:8000/v1"));
    // Host spoof
    assert!(!is_chatgpt_codex_base_url(
        "https://chatgpt.com.evil.invalid/backend-api/codex"
    ));
    assert!(!is_chatgpt_codex_base_url(
        "https://evil.example/?h=chatgpt.com/backend-api/codex"
    ));
}

/// ChatGPT OAuth tokens on disk + active OpenRouter: credentials unchanged,
/// no ChatGPT path, xAI refresh not blocked (but gate is inactive for OR).
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(stored_key_home)]
async fn preturn_chatgpt_connected_openrouter_preserves_provider_key() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let home = tempfile::tempdir().expect("temp home");
            crate::agent::providers::set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
            // Fake ChatGPT OAuth "connected" under the test home.
            crate::auth::chatgpt_oauth::store_tokens(
                home.path(),
                &crate::auth::chatgpt_oauth::ChatGptOAuthTokens {
                    access_token: "chatgpt-access-MUST-NOT-LEAK".into(),
                    refresh_token: "rt".into(),
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(2),
                    account_id: Some("acc".into()),
                    email: None,
                },
            )
            .expect("store chatgpt tokens");
            assert_eq!(
                crate::auth::chatgpt_oauth::status(home.path()),
                crate::auth::chatgpt_oauth::ChatGptOAuthStatus::Connected
            );

            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "openrouter-provider-key".to_string(),
            )
            .await;
            let model_slug = "moonshotai/kimi-k2";
            insert_openrouter_vault_model(&actor, model_slug);
            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .unwrap();
            settings.model = model_slug.to_string();
            settings.base_url = "https://openrouter.ai/api/v1".to_string();
            actor.chat_state_handle.update_inference_settings(settings);

            actor.refresh_token_if_expired().await;

            assert_eq!(
                actor
                    .chat_state_handle
                    .get_credentials()
                    .await
                    .api_key
                    .as_deref(),
                Some("openrouter-provider-key"),
                "ChatGPT OAuth must not overwrite OpenRouter credentials"
            );
            assert!(
                !called.load(Ordering::SeqCst),
                "OpenRouter must not trigger xAI OIDC refresher either"
            );

            crate::agent::providers::set_stored_key_home_for_tests(None);
        })
        .await;
}

/// ChatGPT OAuth connected + active official OpenAI API-key route: key preserved.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(stored_key_home)]
async fn preturn_chatgpt_connected_openai_api_key_preserved() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let home = tempfile::tempdir().expect("temp home");
            crate::agent::providers::set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
            crate::auth::chatgpt_oauth::store_tokens(
                home.path(),
                &crate::auth::chatgpt_oauth::ChatGptOAuthTokens {
                    access_token: "chatgpt-access-MUST-NOT-LEAK".into(),
                    refresh_token: "rt".into(),
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(2),
                    account_id: None,
                    email: None,
                },
            )
            .unwrap();

            let (actor, _rx) = make_actor_with_method_and_credentials(
                None,
                "xai.api_key",
                xai_chat_state::AuthType::ApiKey,
                "openai-api-key-on-wire".to_string(),
            )
            .await;
            insert_openai_api_vault_model(&actor, "gpt-4o");
            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .unwrap();
            settings.model = "gpt-4o".to_string();
            settings.base_url = "https://api.openai.com/v1".to_string();
            actor.chat_state_handle.update_inference_settings(settings);

            actor.refresh_token_if_expired().await;
            assert_eq!(
                actor
                    .chat_state_handle
                    .get_credentials()
                    .await
                    .api_key
                    .as_deref(),
                Some("openai-api-key-on-wire"),
            );

            crate::agent::providers::set_stored_key_home_for_tests(None);
        })
        .await;
}

/// Active ChatGPT Codex route: own OAuth access token is applied to credentials.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(stored_key_home)]
async fn preturn_chatgpt_oauth_active_model_updates_own_credential() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let home = tempfile::tempdir().expect("temp home");
            crate::agent::providers::set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
            crate::auth::chatgpt_oauth::store_tokens(
                home.path(),
                &crate::auth::chatgpt_oauth::ChatGptOAuthTokens {
                    access_token: "chatgpt-fresh-access".into(),
                    refresh_token: "rt".into(),
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(2),
                    account_id: Some("acc".into()),
                    email: None,
                },
            )
            .unwrap();

            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "stale-or-placeholder".to_string(),
            )
            .await;
            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .unwrap();
            settings.model = "gpt-5.3-codex".to_string();
            settings.base_url = crate::auth::chatgpt_oauth::CODEX_RESPONSES_BASE_URL.to_string();
            actor.chat_state_handle.update_inference_settings(settings);

            actor.refresh_token_if_expired().await;
            assert_eq!(
                actor
                    .chat_state_handle
                    .get_credentials()
                    .await
                    .api_key
                    .as_deref(),
                Some("chatgpt-fresh-access"),
                "Codex route must install ChatGPT OAuth access token"
            );
            assert!(
                !called.load(Ordering::SeqCst),
                "ChatGPT OAuth route must not invoke xAI refresher"
            );

            crate::agent::providers::set_stored_key_home_for_tests(None);
        })
        .await;
}

/// Table: credential kind/action distinctions for terminal provider failures.
///
/// | Case | provider | kind | action | generation |
/// |------|----------|------|--------|------------|
/// | OpenRouter API key | openrouter | ApiKey | OpenProviders | nonzero |
/// | OpenAI API key | openai | ApiKey | OpenProviders | nonzero |
/// | ChatGPT OAuth Codex | openai | Oauth | RefreshOauth | nonzero |
/// | xAI session OAuth (terminal, no recovery) | xai | Oauth | RefreshOauth | nonzero |
/// | xAI API key method | xai | ApiKey | OpenProviders | nonzero |
#[tokio::test(flavor = "current_thread")]
async fn provider_credential_kind_action_table() {
    use crate::agent::config::{ModelEntry, ModelInfo};
    use crate::agent::model_providers::{ModelProviderKind, ResolvedModelProvider};
    use crate::extensions::notification::{
        PROVIDER_CREDENTIAL_ERROR_TYPE, ProviderCredentialAction, ProviderCredentialKind,
        SessionUpdate as XaiSessionUpdate,
    };
    use crate::session::storage::SessionUpdate;

    #[derive(Clone, Copy)]
    struct Case {
        name: &'static str,
        auth_method: &'static str,
        auth_type: xai_chat_state::AuthType,
        model: &'static str,
        base_url: &'static str,
        provider_kind: Option<ModelProviderKind>,
        provider_id: &'static str,
        provider_name: &'static str,
        expect_kind: ProviderCredentialKind,
        expect_action: ProviderCredentialAction,
        /// When true, attach an always-failing AuthManager so xAI recovery is
        /// exhausted and the terminal structured event fires.
        xai_terminal: bool,
    }

    let cases = [
        Case {
            name: "openrouter_api_key",
            auth_method: "xai.api_key",
            auth_type: xai_chat_state::AuthType::ApiKey,
            model: "moonshotai/kimi-k2",
            base_url: "https://openrouter.ai/api/v1",
            provider_kind: Some(ModelProviderKind::OpenRouter),
            provider_id: "openrouter",
            provider_name: "OpenRouter",
            expect_kind: ProviderCredentialKind::ApiKey,
            expect_action: ProviderCredentialAction::OpenProviders,
            xai_terminal: false,
        },
        Case {
            name: "openai_api_key",
            auth_method: "xai.api_key",
            auth_type: xai_chat_state::AuthType::ApiKey,
            model: "gpt-4o",
            base_url: "https://api.openai.com/v1",
            provider_kind: Some(ModelProviderKind::OpenAi),
            provider_id: "openai",
            provider_name: "OpenAI",
            expect_kind: ProviderCredentialKind::ApiKey,
            expect_action: ProviderCredentialAction::OpenProviders,
            xai_terminal: false,
        },
        Case {
            name: "chatgpt_oauth",
            auth_method: "xai.api_key",
            auth_type: xai_chat_state::AuthType::ApiKey,
            model: "gpt-5",
            base_url: "https://chatgpt.com/backend-api/codex/responses",
            provider_kind: Some(ModelProviderKind::OpenAi),
            provider_id: "openai",
            provider_name: "OpenAI",
            expect_kind: ProviderCredentialKind::Oauth,
            expect_action: ProviderCredentialAction::RefreshOauth,
            xai_terminal: false,
        },
        Case {
            name: "xai_session_oauth_terminal",
            auth_method: "cached_token",
            auth_type: xai_chat_state::AuthType::SessionToken,
            model: "grok-3",
            base_url: "https://api.x.ai/v1",
            provider_kind: None,
            provider_id: "xai",
            provider_name: "xAI",
            expect_kind: ProviderCredentialKind::Oauth,
            expect_action: ProviderCredentialAction::RefreshOauth,
            xai_terminal: true,
        },
        Case {
            name: "xai_api_key_terminal",
            auth_method: "xai.api_key",
            auth_type: xai_chat_state::AuthType::ApiKey,
            model: "grok-3",
            base_url: "https://api.x.ai/v1",
            provider_kind: None,
            provider_id: "xai",
            provider_name: "xAI",
            expect_kind: ProviderCredentialKind::ApiKey,
            expect_action: ProviderCredentialAction::OpenProviders,
            xai_terminal: true,
        },
    ];

    for case in cases {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (actor, mut persistence_rx) = if case.xai_terminal {
                    // No AuthManager → recovery cannot run; terminal path fires.
                    make_actor_with_method_and_credentials(
                        None,
                        case.auth_method,
                        case.auth_type,
                        "test-key".into(),
                    )
                    .await
                } else {
                    make_actor_with_method_and_credentials(
                        None,
                        case.auth_method,
                        case.auth_type,
                        "test-key".into(),
                    )
                    .await
                };

                if let Some(kind) = case.provider_kind {
                    let mut entry = ModelEntry {
                        info: ModelInfo::fallback(case.model),
                        model_provider: Some(ResolvedModelProvider {
                            id: case.provider_id.to_string(),
                            kind,
                            openrouter_fallback_models: Vec::new(),
                            openrouter_provider_preferences: None,
                            openrouter_plugins: Vec::new(),
                            openrouter_pacing: false,
                            command: Vec::new(),
                        }),
                        api_key: None,
                        env_key: None,
                        auth_provider: None,
                        api_base_url: Some(case.base_url.to_string()),
                    };
                    entry.info.base_url = case.base_url.to_string();
                    entry.info.model = case.model.to_string();
                    actor.models_manager.insert_test_entry(case.model, entry);
                }

                let mut settings = actor
                    .chat_state_handle
                    .get_inference_settings()
                    .await
                    .unwrap();
                settings.model = case.model.to_string();
                settings.base_url = case.base_url.to_string();
                actor.chat_state_handle.update_inference_settings(settings);

                let result = actor.handle_sampling_failure(auth_error()).await;
                assert!(
                    result.is_err(),
                    "{}: expected terminal Err, got Ok(recovery)",
                    case.name
                );

                let mut saw = false;
                while let Ok(msg) = persistence_rx.try_recv() {
                    if let PersistenceMsg::Update(SessionUpdate::Xai(notif)) = msg
                        && let XaiSessionUpdate::RetryState(
                            crate::extensions::notification::RetryState::Failed {
                                error_type,
                                message,
                                provider,
                            },
                        ) = &notif.update
                    {
                        assert_eq!(
                            error_type, PROVIDER_CREDENTIAL_ERROR_TYPE,
                            "{}: error_type",
                            case.name
                        );
                        assert!(
                            !message.contains("/login"),
                            "{}: must not mention /login: {message}",
                            case.name
                        );
                        let p = provider
                            .as_ref()
                            .unwrap_or_else(|| panic!("{}: missing provider context", case.name));
                        assert_eq!(p.provider_id, case.provider_id, "{}", case.name);
                        assert_eq!(p.provider_name, case.provider_name, "{}", case.name);
                        assert_eq!(
                            p.failed_model_id.as_deref(),
                            Some(case.model),
                            "{}",
                            case.name
                        );
                        assert_eq!(p.credential_kind, case.expect_kind, "{}", case.name);
                        assert_eq!(p.recommended_action, case.expect_action, "{}", case.name);
                        assert_ne!(
                            p.credential_generation, 0,
                            "{}: generation must be nonzero",
                            case.name
                        );
                        assert_eq!(p.http_status, Some(401), "{}", case.name);
                        saw = true;
                    }
                }
                assert!(
                    saw,
                    "{}: expected RetryState::Failed with provider",
                    case.name
                );
            })
            .await;
    }
}

/// Generation allocator is monotonic, nonzero, and fails closed without reuse.
#[tokio::test(flavor = "current_thread")]
async fn mint_provider_credential_generation_is_monotonic_and_fails_closed() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _rx) = make_actor_with_auth_and_credentials(
                None,
                xai_chat_state::AuthType::ApiKey,
                "k".into(),
            )
            .await;
            assert_eq!(actor.provider_credential_generation.get(), 0);
            let a = actor.mint_provider_credential_generation().expect("first");
            let b = actor.mint_provider_credential_generation().expect("second");
            assert_eq!(a, 1);
            assert_eq!(b, 2);
            assert_ne!(a, b);

            // Exhaustion: leave counter at MAX so checked_add fails.
            actor.provider_credential_generation.set(u64::MAX);
            assert!(
                actor.mint_provider_credential_generation().is_none(),
                "exhaustion must fail closed"
            );
            assert_eq!(
                actor.provider_credential_generation.get(),
                u64::MAX,
                "counter unchanged on exhaustion"
            );

            // Subsequent build uses reserved 0 (non-resumable) without panic.
            let ctx = actor.synthesize_xai_credential_failure(
                "grok-3",
                "https://api.x.ai/v1",
                Some(401),
                None,
                None,
            );
            assert_eq!(ctx.credential_generation, 0, "exhausted mint → gen 0");
            assert_eq!(ctx.provider_id, "xai");
            assert_eq!(
                ctx.credential_kind,
                crate::extensions::notification::ProviderCredentialKind::ApiKey
            );
        })
        .await;
}

/// ChatGPT OAuth connected + active first-party xAI: xAI refresh still runs;
/// ChatGPT token is not used.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(stored_key_home)]
async fn preturn_chatgpt_connected_first_party_xai_still_refreshes_xai() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let home = tempfile::tempdir().expect("temp home");
            crate::agent::providers::set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
            crate::auth::chatgpt_oauth::store_tokens(
                home.path(),
                &crate::auth::chatgpt_oauth::ChatGptOAuthTokens {
                    access_token: "chatgpt-access-MUST-NOT-LEAK".into(),
                    refresh_token: "rt".into(),
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(2),
                    account_id: None,
                    email: None,
                },
            )
            .unwrap();

            let called = Arc::new(AtomicBool::new(false));
            let refresher: Arc<dyn crate::auth::refresh::TokenRefresher> =
                Arc::new(AlwaysSucceedRefresher {
                    called: called.clone(),
                });
            let (_dir, am) = auth_manager_with_refresher(refresher);
            let (actor, _rx) = make_actor_with_method_and_credentials(
                Some(am),
                "cached_token",
                xai_chat_state::AuthType::SessionToken,
                "initial-test-key".to_string(),
            )
            .await;
            // Default model is first-party; ensure base_url is xAI if set.
            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .unwrap();
            if settings.base_url.is_empty() {
                settings.base_url = "https://api.x.ai/v1".to_string();
                actor.chat_state_handle.update_inference_settings(settings);
            }

            actor.refresh_token_if_expired().await;
            assert!(
                called.load(Ordering::SeqCst),
                "first-party xAI must still invoke OIDC refresher when ChatGPT is connected"
            );
            let key = actor
                .chat_state_handle
                .get_credentials()
                .await
                .api_key
                .expect("key");
            assert_ne!(key, "chatgpt-access-MUST-NOT-LEAK");
            assert_eq!(key, "refreshed-test-token");

            crate::agent::providers::set_stored_key_home_for_tests(None);
        })
        .await;
}
