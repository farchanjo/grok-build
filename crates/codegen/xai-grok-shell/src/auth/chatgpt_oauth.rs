//! Native ChatGPT / OpenAI subscription OAuth (OpenCode/Codex-compatible).
//!
//! Protocol mirrors OpenCode's `plugin/openai/codex.ts`:
//! - Issuer `https://auth.openai.com`
//! - Public Codex client id
//! - Browser authorization-code + PKCE, or headless device auth
//! - Tokens stored under [`OPENAI_OAUTH_SCOPE`] in `auth.json`
//!
//! ChatGPT OAuth and the OpenAI Platform API key use separate credential scopes.
//! They may coexist: the active model route selects OAuth for the ChatGPT Codex
//! endpoint and the API key for `api.openai.com`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, LazyLock, Weak};
use std::time::Duration;

use base64::Engine;
use chrono::{Duration as ChronoDuration, Utc};
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, MutexGuard};

use crate::provider_registry::id::ProviderId;
use crate::provider_registry::instance::ProviderIncarnation;
use crate::provider_registry::secrets::{ProviderOAuthBinding, oauth_scope_string};

use super::model::{AuthMode, GrokAuth};
use super::storage::{
    OPENAI_API_KEY_SCOPE, OPENAI_OAUTH_SCOPE, clear_provider_api_key, clear_provider_oauth_auth,
    read_auth_json, read_auth_json_or_empty, read_provider_oauth_auth, store_provider_oauth_auth,
    write_auth_json,
};

/// Public Codex CLI / OpenCode client id for ChatGPT subscription OAuth.
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const ISSUER: &str = "https://auth.openai.com";
/// Responses base (Grok appends `/responses`).
pub const CODEX_RESPONSES_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
/// Wire headers must match OpenCode/Codex subscription clients (not Grok-branded).
pub const ORIGINATOR: &str = "opencode";

/// Return whether a model route is the ChatGPT Codex subscription endpoint.
///
/// Matching uses the parsed host and path, so a lookalike hostname cannot make
/// an OpenAI API key or another provider route receive a ChatGPT OAuth token.
pub fn is_codex_base_url(base_url: &str) -> bool {
    let Ok(url) = url::Url::parse(base_url) else {
        return false;
    };
    matches!(url.host_str(), Some("chatgpt.com" | "www.chatgpt.com"))
        && url.path().contains("/backend-api/codex")
}

const OAUTH_PORT: u16 = 1455;
const OAUTH_CALLBACK_PATH: &str = "/auth/callback";
const TOKEN_SKEW_SECS: i64 = 60;
const BROWSER_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const DEVICE_POLL_SAFETY_MARGIN: Duration = Duration::from_secs(3);

/// Serialized interactive callback port: `localhost:1455` is the single Codex
/// redirect port, so only one browser login can be active at a time.
static CALLBACK_PORT_LOCK: Mutex<()> = Mutex::const_new(());

/// Acquire the callback port lock, surfacing contention instead of blocking
/// silently so a second login does not look hung.
async fn acquire_callback_port() -> MutexGuard<'static, ()> {
    match CALLBACK_PORT_LOCK.try_lock() {
        Ok(guard) => guard,
        Err(_) => {
            tracing::warn!(
                port = OAUTH_PORT,
                "ChatGPT OAuth login waiting for the previous interactive login to finish"
            );
            CALLBACK_PORT_LOCK.lock().await
        }
    }
}

/// Per-account refresh locks within this process: refreshing one account never
/// blocks another, and concurrent refreshes of the same account are
/// serialized, so a single refresh token is not spent twice in-process. Kept
/// bounded with weak refs: dead entries are pruned when every holder drops.
static REFRESH_LOCKS: LazyLock<Mutex<HashMap<String, Weak<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

async fn refresh_lock_for_scope(scope: &str) -> Arc<Mutex<()>> {
    let mut locks = REFRESH_LOCKS.lock().await;
    locks.retain(|_, w| w.upgrade().is_some());
    if let Some(weak) = locks.get(scope)
        && let Some(arc) = weak.upgrade()
    {
        return arc;
    }
    let arc = Arc::new(Mutex::new(()));
    locks.insert(scope.to_owned(), Arc::downgrade(&arc));
    arc
}

#[derive(Debug, thiserror::Error)]
pub enum ChatGptOAuthError {
    #[error("ChatGPT OAuth credential store is unavailable")]
    Store,
    #[error("ChatGPT OAuth: {0}")]
    Protocol(String),
    #[error("ChatGPT OAuth timed out waiting for browser authorization")]
    Timeout,
    #[error("ChatGPT OAuth cancelled")]
    Cancelled,
    #[error("failed to bind OAuth callback on port {OAUTH_PORT}: {0}")]
    Bind(String),
    #[error("ChatGPT OAuth HTTP error: {0}")]
    Http(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatGptOAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: chrono::DateTime<Utc>,
    pub account_id: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatGptOAuthStatus {
    NotConfigured,
    Connected,
    Expired,
}

/// Which ChatGPT OAuth route a login, refresh, logout, or status targets.
///
/// The built-in OpenAI product route (`openai::oauth`) is the default and
/// keeps full backward compatibility. Configured provider instances use a
/// distinct `provider::<id>::oauth` scope and never fall back to the built-in
/// route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatGptOAuthRoute {
    BuiltIn,
    Configured {
        provider_id: ProviderId,
        incarnation: Option<ProviderIncarnation>,
    },
}

impl ChatGptOAuthRoute {
    /// The auth.json scope key for this route.
    pub fn scope(&self) -> String {
        match self {
            Self::BuiltIn => OPENAI_OAUTH_SCOPE.to_owned(),
            Self::Configured { provider_id, .. } => oauth_scope_string(provider_id),
        }
    }

    pub fn is_configured(&self) -> bool {
        matches!(self, Self::Configured { .. })
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    id_token: Option<String>,
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct IdTokenClaims {
    chatgpt_account_id: Option<String>,
    email: Option<String>,
    organizations: Option<Vec<OrgClaim>>,
    #[serde(rename = "https://api.openai.com/auth")]
    openai_auth: Option<OpenAiAuthClaim>,
}

#[derive(Debug, Deserialize)]
struct OrgClaim {
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiAuthClaim {
    chatgpt_account_id: Option<String>,
}

struct Pkce {
    verifier: String,
    challenge: String,
}

fn base64_url_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn generate_pkce() -> Pkce {
    use sha2::{Digest, Sha256};
    let raw: [u8; 32] = rand::random();
    let verifier = base64_url_encode(&raw);
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = base64_url_encode(&digest);
    Pkce {
        verifier,
        challenge,
    }
}

fn random_state() -> String {
    let raw: [u8; 32] = rand::random();
    base64_url_encode(&raw)
}

pub fn parse_jwt_claims_json(token: &str) -> Option<serde_json::Value> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn extract_account_id_from_tokens(
    id_token: Option<&str>,
    access_token: &str,
) -> Option<String> {
    for token in id_token.into_iter().chain(std::iter::once(access_token)) {
        if let Some(id) = extract_account_id_from_jwt(token) {
            return Some(id);
        }
    }
    None
}

fn extract_account_id_from_jwt(token: &str) -> Option<String> {
    let claims: IdTokenClaims = serde_json::from_value(parse_jwt_claims_json(token)?).ok()?;
    if let Some(id) = claims.chatgpt_account_id {
        return Some(id);
    }
    if let Some(id) = claims.openai_auth.and_then(|a| a.chatgpt_account_id) {
        return Some(id);
    }
    claims
        .organizations
        .and_then(|orgs| orgs.into_iter().find_map(|o| o.id))
}

fn extract_email(id_token: Option<&str>, access_token: &str) -> Option<String> {
    for token in id_token.into_iter().chain(std::iter::once(access_token)) {
        if let Some(claims) = parse_jwt_claims_json(token)
            && let Some(email) = claims.get("email").and_then(|v| v.as_str())
        {
            return Some(email.to_owned());
        }
    }
    None
}

fn tokens_from_response(resp: TokenResponse) -> ChatGptOAuthTokens {
    let account_id = extract_account_id_from_tokens(resp.id_token.as_deref(), &resp.access_token);
    let email = extract_email(resp.id_token.as_deref(), &resp.access_token);
    let expires_in = resp.expires_in.unwrap_or(3600) as i64;
    ChatGptOAuthTokens {
        access_token: resp.access_token,
        refresh_token: resp.refresh_token,
        expires_at: Utc::now() + ChronoDuration::seconds(expires_in),
        account_id,
        email,
    }
}

fn build_authorize_url(redirect_uri: &str, pkce: &Pkce, state: &str) -> String {
    let mut url = url::Url::parse(&format!("{ISSUER}/oauth/authorize")).expect("issuer url");
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("response_type", "code");
        q.append_pair("client_id", CLIENT_ID);
        q.append_pair("redirect_uri", redirect_uri);
        q.append_pair("scope", "openid profile email offline_access");
        q.append_pair("code_challenge", &pkce.challenge);
        q.append_pair("code_challenge_method", "S256");
        q.append_pair("id_token_add_organizations", "true");
        q.append_pair("codex_cli_simplified_flow", "true");
        q.append_pair("state", state);
        q.append_pair("originator", ORIGINATOR);
    }
    url.to_string()
}

async fn exchange_code(
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<ChatGptOAuthTokens, ChatGptOAuthError> {
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", CLIENT_ID),
        ("code_verifier", code_verifier),
    ]
    .iter()
    .map(|(k, v)| format!("{}={}", k, urlencoding_encode(v)))
    .collect::<Vec<_>>()
    .join("&");
    let client = xai_grok_tools::extra_ca::with_extra_root_certificates(reqwest::Client::builder())
        .build()
        .map_err(|e| ChatGptOAuthError::Http(e.to_string()))?;
    let response = client
        .post(format!("{ISSUER}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form)
        .send()
        .await
        .map_err(|e| ChatGptOAuthError::Http(e.to_string()))?;
    if !response.status().is_success() {
        return Err(ChatGptOAuthError::Protocol(format!(
            "token exchange failed: {}",
            response.status()
        )));
    }
    let body: TokenResponse = response
        .json()
        .await
        .map_err(|e| ChatGptOAuthError::Protocol(e.to_string()))?;
    Ok(tokens_from_response(body))
}

fn urlencoding_encode(s: &str) -> String {
    // application/x-www-form-urlencoded percent-encoding for token fields.
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Test-only IdP refresh double. Activation is an explicit `Some` handler:
/// once a call clones this `Arc`, it never re-checks the global slot and
/// cannot fall through to live HTTP if another test mutates process state.
#[cfg(test)]
struct TestRefreshStub {
    delay: Duration,
    entries: std::sync::atomic::AtomicU32,
}

/// Process-global slot for the active refresh double. Installers are
/// serialized with other ChatGPT OAuth process-global tests via
/// `#[serial_test::serial(chatgpt_oauth_test_globals)]`; cleared by
/// [`TestRefreshGuard`]'s `Drop` even on panic.
#[cfg(test)]
static TEST_REFRESH_STUB: std::sync::Mutex<Option<Arc<TestRefreshStub>>> =
    std::sync::Mutex::new(None);

/// RAII install of the hermetic refresh double. Restores `None` on drop so a
/// panic cannot leave the stub enabled for later shared-process tests.
#[cfg(test)]
struct TestRefreshGuard {
    stub: Arc<TestRefreshStub>,
}

#[cfg(test)]
impl TestRefreshGuard {
    fn install(delay: Duration) -> Self {
        let stub = Arc::new(TestRefreshStub {
            delay,
            entries: std::sync::atomic::AtomicU32::new(0),
        });
        let mut slot = TEST_REFRESH_STUB.lock().unwrap();
        assert!(
            slot.is_none(),
            "refresh stub already installed; tests must be serial on chatgpt_oauth_test_globals"
        );
        *slot = Some(stub.clone());
        Self { stub }
    }

    fn entries(&self) -> u32 {
        self.stub.entries.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
impl Drop for TestRefreshGuard {
    fn drop(&mut self) {
        *TEST_REFRESH_STUB.lock().unwrap() = None;
    }
}

async fn refresh_with_token(refresh_token: &str) -> Result<ChatGptOAuthTokens, ChatGptOAuthError> {
    #[cfg(test)]
    {
        let stub = TEST_REFRESH_STUB.lock().unwrap().clone();
        if let Some(stub) = stub {
            stub.entries
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tokio::time::sleep(stub.delay).await;
            return Ok(ChatGptOAuthTokens {
                access_token: format!("refreshed-{refresh_token}"),
                refresh_token: format!("rotated-{refresh_token}"),
                expires_at: Utc::now() + ChronoDuration::hours(1),
                account_id: Some("acc-refreshed".into()),
                email: None,
            });
        }
    }
    let form = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}",
        urlencoding_encode(refresh_token),
        urlencoding_encode(CLIENT_ID)
    );
    let client = xai_grok_tools::extra_ca::with_extra_root_certificates(reqwest::Client::builder())
        .build()
        .map_err(|e| ChatGptOAuthError::Http(e.to_string()))?;
    let response = client
        .post(format!("{ISSUER}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form)
        .send()
        .await
        .map_err(|e| ChatGptOAuthError::Http(e.to_string()))?;
    if !response.status().is_success() {
        return Err(ChatGptOAuthError::Protocol(format!(
            "token refresh failed: {}",
            response.status()
        )));
    }
    let body: TokenResponse = response
        .json()
        .await
        .map_err(|e| ChatGptOAuthError::Protocol(e.to_string()))?;
    Ok(tokens_from_response(body))
}

fn auth_to_tokens(auth: &GrokAuth) -> Option<ChatGptOAuthTokens> {
    let refresh = auth.refresh_token.clone()?;
    Some(ChatGptOAuthTokens {
        access_token: auth.key.clone(),
        refresh_token: refresh,
        expires_at: auth.expires_at.unwrap_or_else(Utc::now),
        account_id: auth.organization_id.clone(),
        email: auth.email.clone(),
    })
}

fn tokens_to_auth(tokens: &ChatGptOAuthTokens) -> GrokAuth {
    GrokAuth {
        key: tokens.access_token.clone(),
        auth_mode: AuthMode::Oidc,
        create_time: Utc::now(),
        user_id: tokens.email.clone().unwrap_or_else(|| "chatgpt".to_owned()),
        email: tokens.email.clone(),
        refresh_token: Some(tokens.refresh_token.clone()),
        expires_at: Some(tokens.expires_at),
        oidc_issuer: Some(ISSUER.to_owned()),
        oidc_client_id: Some(CLIENT_ID.to_owned()),
        organization_id: tokens.account_id.clone(),
        coding_data_retention_opt_out: true,
        ..Default::default()
    }
}

/// Route-scoped storage helpers. Built-in never falls back to a configured
/// scope and vice versa.
fn read_route_auth(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
) -> Result<Option<GrokAuth>, ChatGptOAuthError> {
    match route {
        ChatGptOAuthRoute::BuiltIn => match read_auth_json(&grok_home.join("auth.json")) {
            Ok(store) => Ok(store.get(OPENAI_OAUTH_SCOPE).cloned()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(ChatGptOAuthError::Store),
        },
        ChatGptOAuthRoute::Configured {
            provider_id,
            incarnation,
        } => read_provider_oauth_auth(grok_home, provider_id, incarnation.as_ref())
            .map_err(|_| ChatGptOAuthError::Store),
    }
}

/// Secret-free binding sidecar for the built-in `openai::oauth` token scope.
const OPENAI_OAUTH_META_SCOPE: &str = "openai::oauth::meta";
const OPENAI_OAUTH_META_MARKER: &str = "meta";

fn encode_builtin_oauth_meta(generation: u64) -> GrokAuth {
    GrokAuth {
        key: OPENAI_OAUTH_META_MARKER.to_owned(),
        auth_mode: AuthMode::ApiKey,
        create_time: chrono::Utc::now(),
        user_id: generation.to_string(),
        ..Default::default()
    }
}

fn decode_builtin_oauth_meta(entry: &GrokAuth) -> std::io::Result<u64> {
    if entry.key != OPENAI_OAUTH_META_MARKER || entry.refresh_token.is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "malformed built-in ChatGPT OAuth binding record",
        ));
    }
    entry
        .user_id
        .parse::<u64>()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Read the durable secret-free binding generation for built-in ChatGPT OAuth.
pub fn read_builtin_oauth_binding_generation(grok_home: &Path) -> std::io::Result<Option<u64>> {
    let path = grok_home.join("auth.json");
    let store = match read_auth_json(&path) {
        Ok(store) => store,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    match store.get(OPENAI_OAUTH_META_SCOPE) {
        Some(meta) => decode_builtin_oauth_meta(meta).map(Some),
        None => Ok(None),
    }
}

/// Persist route auth, preserving `WouldBlock` so post-refresh callers can
/// retry lock contention without dropping a just-rotated refresh token.
///
/// Returns the exact durable binding generation committed by this write.
fn store_route_auth_io(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
    auth: &GrokAuth,
) -> std::io::Result<u64> {
    match route {
        ChatGptOAuthRoute::BuiltIn => {
            let path = grok_home.join("auth.json");
            let lock = crate::auth::manager::lock::try_lock_auth_file_nonblocking(&path)
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::WouldBlock))?;
            if !lock.still_live(&path) {
                return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
            }
            let mut store = read_auth_json_or_empty(&path)?;
            let prev_generation = match store.get(OPENAI_OAUTH_META_SCOPE) {
                Some(meta) => decode_builtin_oauth_meta(meta)?,
                None => 0,
            };
            let generation = prev_generation.saturating_add(1).max(1);
            store.insert(OPENAI_OAUTH_SCOPE.to_owned(), auth.clone());
            store.insert(
                OPENAI_OAUTH_META_SCOPE.to_owned(),
                encode_builtin_oauth_meta(generation),
            );
            if !lock.still_live(&path) {
                return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
            }
            write_auth_json(&path, &store)?;
            Ok(generation)
        }
        ChatGptOAuthRoute::Configured {
            provider_id,
            incarnation,
        } => store_provider_oauth_auth(grok_home, provider_id, incarnation.as_ref(), auth)
            .map(|binding| binding.generation),
    }
}

fn store_route_auth(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
    auth: &GrokAuth,
) -> Result<u64, ChatGptOAuthError> {
    store_route_auth_io(grok_home, route, auth).map_err(|_| ChatGptOAuthError::Store)
}

/// Bounded wait for post-refresh persist under auth-file contention.
/// Keeps total wait finite and yields between attempts (no busy loop).
const POST_REFRESH_STORE_TIMEOUT: Duration = Duration::from_secs(5);
const POST_REFRESH_STORE_STEP: Duration = Duration::from_millis(20);

/// Persist tokens after a successful IdP rotation. Retries only
/// `WouldBlock` so a just-issued refresh token is not discarded on
/// transient auth-file contention; non-contention I/O fails fast.
async fn store_tokens_route_after_refresh(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
    tokens: &ChatGptOAuthTokens,
) -> Result<(), ChatGptOAuthError> {
    let auth = tokens_to_auth(tokens);
    let deadline = tokio::time::Instant::now() + POST_REFRESH_STORE_TIMEOUT;
    loop {
        match store_route_auth_io(grok_home, route, &auth) {
            Ok(_generation) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if tokio::time::Instant::now() >= deadline {
                    return Err(ChatGptOAuthError::Store);
                }
                // Cancellation-safe backoff; do not hold a file lock across waits.
                tokio::time::sleep(POST_REFRESH_STORE_STEP).await;
            }
            Err(_) => return Err(ChatGptOAuthError::Store),
        }
    }
}

fn clear_route_auth(grok_home: &Path, route: &ChatGptOAuthRoute) -> Result<(), ChatGptOAuthError> {
    match route {
        ChatGptOAuthRoute::BuiltIn => {
            let path = grok_home.join("auth.json");
            let lock = crate::auth::manager::lock::try_lock_auth_file_nonblocking(&path)
                .ok_or(ChatGptOAuthError::Store)?;
            if !lock.still_live(&path) {
                return Err(ChatGptOAuthError::Store);
            }
            let mut store = match read_auth_json(&path) {
                Ok(store) => store,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(_) => return Err(ChatGptOAuthError::Store),
            };
            store.remove(OPENAI_OAUTH_SCOPE);
            if !lock.still_live(&path) {
                return Err(ChatGptOAuthError::Store);
            }
            write_auth_json(&path, &store).map_err(|_| ChatGptOAuthError::Store)
        }
        ChatGptOAuthRoute::Configured {
            provider_id,
            incarnation,
        } => clear_provider_oauth_auth(grok_home, provider_id, incarnation.as_ref())
            .map_err(|_| ChatGptOAuthError::Store),
    }
}

/// Persist OAuth tokens for a specific route/account without changing the
/// separately scoped OpenAI API key. A configured account's tokens never touch
/// the built-in `openai::oauth` entry and vice versa.
pub fn store_tokens_route(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
    tokens: &ChatGptOAuthTokens,
) -> Result<(), ChatGptOAuthError> {
    store_tokens_route_generation(grok_home, route, tokens).map(|_| ())
}

/// Like [`store_tokens_route`], but returns the exact durable binding
/// generation committed by the store (for operation-bound repair receipts).
pub fn store_tokens_route_generation(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
    tokens: &ChatGptOAuthTokens,
) -> Result<u64, ChatGptOAuthError> {
    store_route_auth(grok_home, route, &tokens_to_auth(tokens))
}

/// Clear ChatGPT OAuth tokens for a specific route/account. Only the selected
/// account is removed.
pub fn clear_tokens_route(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
) -> Result<(), ChatGptOAuthError> {
    clear_route_auth(grok_home, route)
}

/// Read ChatGPT OAuth tokens for a specific route/account.
pub fn read_tokens_route(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
) -> Result<Option<ChatGptOAuthTokens>, ChatGptOAuthError> {
    Ok(read_route_auth(grok_home, route)?.and_then(|auth| auth_to_tokens(&auth)))
}

/// ChatGPT OAuth status for a specific route/account.
pub fn status_route(grok_home: &Path, route: &ChatGptOAuthRoute) -> ChatGptOAuthStatus {
    match read_tokens_route(grok_home, route) {
        Ok(Some(tokens)) => {
            if tokens.expires_at <= Utc::now() + ChronoDuration::seconds(TOKEN_SKEW_SECS)
                && tokens.refresh_token.is_empty()
            {
                ChatGptOAuthStatus::Expired
            } else {
                ChatGptOAuthStatus::Connected
            }
        }
        Ok(None) | Err(_) => ChatGptOAuthStatus::NotConfigured,
    }
}

/// The persisted secret-free binding of a configured route when it matches the
/// route's incarnation exactly (`None` ≠ `Some`, UUID must match). Survives
/// token deletion so generation stays monotonic across logout/login. Built-in
/// routes and incarnation mismatches return `None`.
pub fn oauth_route_binding(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
) -> Result<Option<ProviderOAuthBinding>, ChatGptOAuthError> {
    let ChatGptOAuthRoute::Configured {
        provider_id,
        incarnation,
    } = route
    else {
        return Ok(None);
    };
    let Some(binding) = crate::auth::storage::read_provider_oauth_binding(grok_home, provider_id)
        .map_err(|_| ChatGptOAuthError::Store)?
    else {
        return Ok(None);
    };
    if binding.incarnation.as_ref() != incarnation.as_ref() {
        return Ok(None);
    }
    Ok(Some(binding))
}

/// Backward-compatible built-in wrappers. These keep the exact pre-PR2
/// behavior and are the default used by all existing callers.
pub fn store_tokens(
    grok_home: &Path,
    tokens: &ChatGptOAuthTokens,
) -> Result<(), ChatGptOAuthError> {
    store_tokens_route(grok_home, &ChatGptOAuthRoute::BuiltIn, tokens)
}

pub fn clear_tokens(grok_home: &Path) -> Result<(), ChatGptOAuthError> {
    clear_tokens_route(grok_home, &ChatGptOAuthRoute::BuiltIn)
}

pub fn read_tokens(grok_home: &Path) -> Result<Option<ChatGptOAuthTokens>, ChatGptOAuthError> {
    read_tokens_route(grok_home, &ChatGptOAuthRoute::BuiltIn)
}

pub fn status(grok_home: &Path) -> ChatGptOAuthStatus {
    status_route(grok_home, &ChatGptOAuthRoute::BuiltIn)
}

/// Return a usable access token for a specific route, refreshing when near
/// expiry. Refresh is serialized per account scope.
pub async fn valid_access_token_route(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
) -> Result<Option<(String, Option<String>)>, ChatGptOAuthError> {
    let Some(tokens) = read_tokens_route(grok_home, route)? else {
        return Ok(None);
    };
    let near_expiry = tokens.expires_at <= Utc::now() + ChronoDuration::seconds(TOKEN_SKEW_SECS);
    if !near_expiry {
        return Ok(Some((tokens.access_token, tokens.account_id)));
    }
    let scope = route.scope();
    // Retain the Arc and hold the inner mutex across re-read / refresh / store
    // so the same refresh token is never spent twice in-process.
    let lock = refresh_lock_for_scope(&scope).await;
    let _guard = lock.lock().await;
    // Re-read after lock in case another task refreshed this account.
    let Some(tokens) = read_tokens_route(grok_home, route)? else {
        return Ok(None);
    };
    if tokens.expires_at > Utc::now() + ChronoDuration::seconds(TOKEN_SKEW_SECS) {
        return Ok(Some((tokens.access_token, tokens.account_id)));
    }
    let refreshed = refresh_with_token(&tokens.refresh_token).await?;
    // Preserve account id if refresh response omits it.
    let mut refreshed = refreshed;
    if refreshed.account_id.is_none() {
        refreshed.account_id = tokens.account_id;
    }
    // Bounded retry on auth-file contention: the IdP already rotated the
    // refresh token; do not discard it on a transient WouldBlock.
    store_tokens_route_after_refresh(grok_home, route, &refreshed).await?;
    Ok(Some((refreshed.access_token, refreshed.account_id)))
}

/// Backward-compatible built-in wrapper (default route).
pub async fn valid_access_token(
    grok_home: &Path,
) -> Result<Option<(String, Option<String>)>, ChatGptOAuthError> {
    valid_access_token_route(grok_home, &ChatGptOAuthRoute::BuiltIn).await
}

/// Store an OpenAI Platform API key without changing ChatGPT OAuth tokens.
pub fn store_api_key(grok_home: &Path, api_key: &str) -> Result<(), ChatGptOAuthError> {
    crate::auth::store_provider_api_key(grok_home, OPENAI_API_KEY_SCOPE, api_key)
        .map(|_| ())
        .map_err(|_| ChatGptOAuthError::Store)
}

pub fn clear_api_key_only(grok_home: &Path) -> Result<(), ChatGptOAuthError> {
    clear_provider_api_key(grok_home, OPENAI_API_KEY_SCOPE).map_err(|_| ChatGptOAuthError::Store)
}

async fn open_browser(url: &str) -> bool {
    let url = url.to_owned();
    matches!(
        tokio::task::spawn_blocking(move || webbrowser::open(&url)).await,
        Ok(Ok(()))
    )
}

fn success_html() -> &'static str {
    "<!DOCTYPE html><html><body style=\"font-family:sans-serif;text-align:center;padding:3rem\">\
     <h1>ChatGPT connected</h1><p>You can close this window and return to Grok Build.</p>\
     </body></html>"
}

fn error_html(msg: &str) -> String {
    format!(
        "<!DOCTYPE html><html><body style=\"font-family:sans-serif;text-align:center;padding:3rem\">\
         <h1>Authorization failed</h1><p>{}</p></body></html>",
        html_escape(msg)
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Exact equality check for the per-login random CSRF `state`.
fn callback_state_matches(got: Option<&str>, expected: &str) -> bool {
    got == Some(expected)
}

/// Browser PKCE login for a specific route/account. Binds `localhost:1455`
/// (Codex redirect_uri). The interactive callback port is globally serialized;
/// each callback's `state` is a per-login random value.
///
/// Returns tokens and the exact durable binding generation from the store write.
pub async fn login_browser_route_generation(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
) -> Result<(ChatGptOAuthTokens, u64), ChatGptOAuthError> {
    let _port_guard = acquire_callback_port().await;
    let redirect_uri = format!("http://localhost:{OAUTH_PORT}{OAUTH_CALLBACK_PATH}");
    let pkce = generate_pkce();
    let state = random_state();
    let auth_url = build_authorize_url(&redirect_uri, &pkce, &state);

    let listener = TcpListener::bind(("127.0.0.1", OAUTH_PORT))
        .await
        .map_err(|e| ChatGptOAuthError::Bind(e.to_string()))?;

    let _ = open_browser(&auth_url).await;

    let tokens = tokio::time::timeout(BROWSER_TIMEOUT, async {
        loop {
            let (mut socket, _) = listener
                .accept()
                .await
                .map_err(|e| ChatGptOAuthError::Http(e.to_string()))?;
            let mut reader = BufReader::new(&mut socket);
            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .await
                .map_err(|e| ChatGptOAuthError::Http(e.to_string()))?;
            // Drain headers
            loop {
                let mut line = String::new();
                reader
                    .read_line(&mut line)
                    .await
                    .map_err(|e| ChatGptOAuthError::Http(e.to_string()))?;
                if line == "\r\n" || line == "\n" || line.is_empty() {
                    break;
                }
            }

            let path = request_line.split_whitespace().nth(1).unwrap_or("/");
            let url = url::Url::parse(&format!("http://localhost{path}"))
                .map_err(|e| ChatGptOAuthError::Protocol(e.to_string()))?;

            if url.path() == "/cancel" {
                let body = "Login cancelled";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                return Err(ChatGptOAuthError::Cancelled);
            }

            if url.path() != OAUTH_CALLBACK_PATH {
                let body = "Not found";
                let resp = format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                continue;
            }

            if let Some(err) = url.query_pairs().find(|(k, _)| k == "error").map(|(_, v)| v) {
                let desc = url
                    .query_pairs()
                    .find(|(k, _)| k == "error_description")
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_else(|| err.to_string());
                let body = error_html(&desc);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                return Err(ChatGptOAuthError::Protocol(desc));
            }

            let code = url
                .query_pairs()
                .find(|(k, _)| k == "code")
                .map(|(_, v)| v.to_string());
            let got_state = url
                .query_pairs()
                .find(|(k, _)| k == "state")
                .map(|(_, v)| v.to_string());

            if !callback_state_matches(got_state.as_deref(), &state) {
                let body = error_html("Invalid state");
                let resp = format!(
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                return Err(ChatGptOAuthError::Protocol(
                    "Invalid state - potential CSRF".into(),
                ));
            }

            let Some(code) = code else {
                let body = error_html("Missing authorization code");
                let resp = format!(
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                return Err(ChatGptOAuthError::Protocol("Missing authorization code".into()));
            };

            let tokens = exchange_code(&code, &redirect_uri, &pkce.verifier).await?;
            let body = success_html();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(resp.as_bytes()).await;
            return Ok(tokens);
        }
    })
    .await
    .map_err(|_| ChatGptOAuthError::Timeout)??;

    let generation = store_tokens_route_generation(grok_home, route, &tokens)?;
    Ok((tokens, generation))
}

/// Browser PKCE login for a specific route/account (tokens only).
pub async fn login_browser_route(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
) -> Result<ChatGptOAuthTokens, ChatGptOAuthError> {
    login_browser_route_generation(grok_home, route)
        .await
        .map(|(tokens, _)| tokens)
}

/// Backward-compatible built-in wrapper (default route).
pub async fn login_browser(grok_home: &Path) -> Result<ChatGptOAuthTokens, ChatGptOAuthError> {
    login_browser_route(grok_home, &ChatGptOAuthRoute::BuiltIn).await
}

#[derive(Debug, Deserialize)]
struct DeviceUserCodeResponse {
    device_auth_id: String,
    user_code: String,
    #[serde(default)]
    interval: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeviceTokenResponse {
    authorization_code: String,
    code_verifier: String,
}

/// Headless device-code login. Returns the user-facing code and verification URL
/// while polling in the background via [`complete_device_login`].
pub struct DeviceLoginStart {
    pub user_code: String,
    pub verification_url: String,
    pub device_auth_id: String,
    pub interval: Duration,
}

pub async fn start_device_login() -> Result<DeviceLoginStart, ChatGptOAuthError> {
    let client = xai_grok_tools::extra_ca::with_extra_root_certificates(reqwest::Client::builder())
        .build()
        .map_err(|e| ChatGptOAuthError::Http(e.to_string()))?;
    let response = client
        .post(format!("{ISSUER}/api/accounts/deviceauth/usercode"))
        .header("Content-Type", "application/json")
        .header(
            "User-Agent",
            format!("opencode/{}", env!("CARGO_PKG_VERSION")),
        )
        .json(&serde_json::json!({ "client_id": CLIENT_ID }))
        .send()
        .await
        .map_err(|e| ChatGptOAuthError::Http(e.to_string()))?;
    if !response.status().is_success() {
        return Err(ChatGptOAuthError::Protocol(format!(
            "device auth start failed: {}",
            response.status()
        )));
    }
    let data: DeviceUserCodeResponse = response
        .json()
        .await
        .map_err(|e| ChatGptOAuthError::Protocol(e.to_string()))?;
    let interval_secs = data
        .interval
        .as_deref()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(5)
        .max(1);
    Ok(DeviceLoginStart {
        user_code: data.user_code,
        verification_url: format!("{ISSUER}/codex/device"),
        device_auth_id: data.device_auth_id,
        interval: Duration::from_secs(interval_secs),
    })
}

/// Poll device login to completion for a specific route/account. Tokens are
/// stored under the route's scope so a configured account never touches the
/// built-in entry. Returns the exact store-committed binding generation.
pub async fn complete_device_login_route_generation(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
    start: &DeviceLoginStart,
) -> Result<(ChatGptOAuthTokens, u64), ChatGptOAuthError> {
    let client = xai_grok_tools::extra_ca::with_extra_root_certificates(reqwest::Client::builder())
        .build()
        .map_err(|e| ChatGptOAuthError::Http(e.to_string()))?;
    let deadline = tokio::time::Instant::now() + BROWSER_TIMEOUT;
    loop {
        if tokio::time::Instant::now() > deadline {
            return Err(ChatGptOAuthError::Timeout);
        }
        let response = client
            .post(format!("{ISSUER}/api/accounts/deviceauth/token"))
            .header("Content-Type", "application/json")
            .header(
                "User-Agent",
                format!("opencode/{}", env!("CARGO_PKG_VERSION")),
            )
            .json(&serde_json::json!({
                "device_auth_id": start.device_auth_id,
                "user_code": start.user_code,
            }))
            .send()
            .await
            .map_err(|e| ChatGptOAuthError::Http(e.to_string()))?;

        if response.status().is_success() {
            let data: DeviceTokenResponse = response
                .json()
                .await
                .map_err(|e| ChatGptOAuthError::Protocol(e.to_string()))?;
            let tokens = exchange_code(
                &data.authorization_code,
                &format!("{ISSUER}/deviceauth/callback"),
                &data.code_verifier,
            )
            .await?;
            let generation = store_tokens_route_generation(grok_home, route, &tokens)?;
            return Ok((tokens, generation));
        }

        if response.status().as_u16() != 403 && response.status().as_u16() != 404 {
            return Err(ChatGptOAuthError::Protocol(format!(
                "device auth poll failed: {}",
                response.status()
            )));
        }

        tokio::time::sleep(start.interval + DEVICE_POLL_SAFETY_MARGIN).await;
    }
}

/// Poll device login to completion; returns tokens only.
pub async fn complete_device_login_route(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
    start: &DeviceLoginStart,
) -> Result<ChatGptOAuthTokens, ChatGptOAuthError> {
    complete_device_login_route_generation(grok_home, route, start)
        .await
        .map(|(tokens, _)| tokens)
}

/// Backward-compatible built-in wrapper (default route).
pub async fn complete_device_login(
    grok_home: &Path,
    start: &DeviceLoginStart,
) -> Result<ChatGptOAuthTokens, ChatGptOAuthError> {
    complete_device_login_route(grok_home, &ChatGptOAuthRoute::BuiltIn, start).await
}

/// Full device login for a specific route/account: start, print the user code,
/// open browser, poll to completion.
///
/// OpenAI's verification page requires the user to type the 9-character code
/// shown in the terminal. Always surface it on stderr (OpenCode equivalent of
/// `instructions: Enter code: …`); `tracing` alone is invisible in normal use.
pub async fn login_device_route(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
) -> Result<ChatGptOAuthTokens, ChatGptOAuthError> {
    login_device_route_generation(grok_home, route)
        .await
        .map(|(tokens, _)| tokens)
}

/// Device login that also returns the exact store-committed binding generation.
pub async fn login_device_route_generation(
    grok_home: &Path,
    route: &ChatGptOAuthRoute,
) -> Result<(ChatGptOAuthTokens, u64), ChatGptOAuthError> {
    let start = start_device_login().await?;
    // Print before opening the browser so the code is visible if focus moves.
    eprintln!();
    eprintln!("ChatGPT device login");
    eprintln!("  Open:  {}", start.verification_url);
    eprintln!("  Code:  {}", start.user_code);
    eprintln!("Enter the code in the browser, then return here.");
    eprintln!();
    tracing::info!(
        user_code = %start.user_code,
        url = %start.verification_url,
        "ChatGPT device login: enter the code in the browser"
    );
    let _ = open_browser(&start.verification_url).await;
    complete_device_login_route_generation(grok_home, route, &start).await
}

/// Backward-compatible built-in wrapper (default route).
pub async fn login_device(grok_home: &Path) -> Result<ChatGptOAuthTokens, ChatGptOAuthError> {
    login_device_route(grok_home, &ChatGptOAuthRoute::BuiltIn).await
}

/// Extra headers for ChatGPT OAuth inference (Codex/OpenAI wire shape).
pub fn oauth_extra_headers(account_id: Option<&str>) -> indexmap::IndexMap<String, String> {
    let mut headers = indexmap::IndexMap::new();
    headers.insert("originator".to_owned(), ORIGINATOR.to_owned());
    headers.insert(
        "User-Agent".to_owned(),
        format!(
            "opencode/{} ({} {})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    );
    if let Some(id) = account_id.filter(|s| !s.is_empty()) {
        headers.insert("ChatGPT-Account-Id".to_owned(), id.to_owned());
    }
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_account_id_from_nested_claim() {
        // header.payload.sig — payload is {"https://api.openai.com/auth":{"chatgpt_account_id":"acc-1"}}
        let payload =
            base64_url_encode(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"acc-1"}}"#);
        let jwt = format!("hdr.{payload}.sig");
        assert_eq!(extract_account_id_from_jwt(&jwt).as_deref(), Some("acc-1"));
    }

    #[test]
    fn extracts_top_level_account_id() {
        let payload = base64_url_encode(br#"{"chatgpt_account_id":"acc-2","email":"a@b.c"}"#);
        let jwt = format!("hdr.{payload}.sig");
        assert_eq!(extract_account_id_from_jwt(&jwt).as_deref(), Some("acc-2"));
    }

    #[test]
    fn authorize_url_contains_pkce_and_originator() {
        let pkce = Pkce {
            verifier: "v".into(),
            challenge: "c".into(),
        };
        let url = build_authorize_url("http://localhost:1455/auth/callback", &pkce, "st");
        assert!(url.contains("code_challenge=c"));
        assert!(url.contains("originator=opencode"));
        assert!(url.contains("codex_cli_simplified_flow=true"));
        assert!(url.contains("client_id=app_EMoamEEZ73f0CkXaXp7hrann"));
    }

    #[test]
    fn store_and_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let tokens = ChatGptOAuthTokens {
            access_token: "access".into(),
            refresh_token: "refresh".into(),
            expires_at: Utc::now() + ChronoDuration::hours(1),
            account_id: Some("acc".into()),
            email: Some("u@x.ai".into()),
        };
        store_tokens(dir.path(), &tokens).unwrap();
        let loaded = read_tokens(dir.path()).unwrap().unwrap();
        assert_eq!(loaded.access_token, "access");
        assert_eq!(loaded.account_id.as_deref(), Some("acc"));
        assert_eq!(status(dir.path()), ChatGptOAuthStatus::Connected);
    }

    #[test]
    fn oauth_store_preserves_api_key() {
        let dir = tempfile::tempdir().unwrap();
        crate::auth::store_provider_api_key(dir.path(), OPENAI_API_KEY_SCOPE, "sk-test").unwrap();
        let tokens = ChatGptOAuthTokens {
            access_token: "access".into(),
            refresh_token: "refresh".into(),
            expires_at: Utc::now() + ChronoDuration::hours(1),
            account_id: None,
            email: None,
        };
        store_tokens(dir.path(), &tokens).unwrap();
        assert_eq!(
            crate::auth::read_provider_api_key(dir.path(), OPENAI_API_KEY_SCOPE)
                .unwrap()
                .as_deref(),
            Some("sk-test")
        );
    }

    #[test]
    fn api_key_store_preserves_oauth() {
        let dir = tempfile::tempdir().unwrap();
        let tokens = ChatGptOAuthTokens {
            access_token: "access".into(),
            refresh_token: "refresh".into(),
            expires_at: Utc::now() + ChronoDuration::hours(1),
            account_id: None,
            email: None,
        };
        store_tokens(dir.path(), &tokens).unwrap();
        store_api_key(dir.path(), "sk-new").unwrap();
        assert!(read_tokens(dir.path()).unwrap().is_some());
        assert_eq!(
            crate::auth::read_provider_api_key(dir.path(), OPENAI_API_KEY_SCOPE)
                .unwrap()
                .as_deref(),
            Some("sk-new")
        );
    }

    // ── Route-aware multi-account behavior ────────────────────────────────

    fn route_tokens(access: &str) -> ChatGptOAuthTokens {
        ChatGptOAuthTokens {
            access_token: access.into(),
            refresh_token: format!("rt-{access}"),
            expires_at: Utc::now() + ChronoDuration::hours(1),
            account_id: Some(format!("acc-{access}")),
            email: None,
        }
    }

    fn configured_route(id: &str) -> ChatGptOAuthRoute {
        ChatGptOAuthRoute::Configured {
            provider_id: ProviderId::new(id).unwrap(),
            incarnation: None,
        }
    }

    #[test]
    fn route_store_read_roundtrip_and_binding() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc = ProviderIncarnation::new("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let route = ChatGptOAuthRoute::Configured {
            provider_id: ProviderId::new("corp").unwrap(),
            incarnation: Some(inc.clone()),
        };

        store_tokens_route(home, &route, &route_tokens("route-access")).unwrap();
        let loaded = read_tokens_route(home, &route).unwrap().unwrap();
        assert_eq!(loaded.access_token, "route-access");
        assert_eq!(loaded.account_id.as_deref(), Some("acc-route-access"));
        assert_eq!(status_route(home, &route), ChatGptOAuthStatus::Connected);

        let binding = oauth_route_binding(home, &route).unwrap().unwrap();
        assert_eq!(binding.generation, 0);
        assert_eq!(binding.provider_id.as_str(), "corp");
        assert_eq!(binding.incarnation.as_ref(), Some(&inc));

        // Built-in route must not see configured tokens and vice versa.
        assert!(read_tokens(home).unwrap().is_none());
        assert!(
            oauth_route_binding(home, &ChatGptOAuthRoute::BuiltIn)
                .unwrap()
                .is_none()
        );

        // Replace: generation rotates, only this account is touched.
        store_tokens_route(home, &route, &route_tokens("route-access-2")).unwrap();
        assert_eq!(
            oauth_route_binding(home, &route)
                .unwrap()
                .unwrap()
                .generation,
            1
        );
        assert_eq!(
            read_tokens_route(home, &route)
                .unwrap()
                .unwrap()
                .access_token,
            "route-access-2"
        );

        clear_tokens_route(home, &route).unwrap();
        assert!(read_tokens_route(home, &route).unwrap().is_none());
        // The binding record and rotated generation survive logout.
        assert_eq!(
            oauth_route_binding(home, &route)
                .unwrap()
                .unwrap()
                .generation,
            2
        );
    }

    #[test]
    fn route_store_never_crosses_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let configured = configured_route("corp");

        store_tokens(home, &route_tokens("builtin-access")).unwrap();
        store_tokens_route(home, &configured, &route_tokens("configured-access")).unwrap();

        assert_eq!(
            read_tokens(home).unwrap().unwrap().access_token,
            "builtin-access"
        );
        assert_eq!(
            read_tokens_route(home, &configured)
                .unwrap()
                .unwrap()
                .access_token,
            "configured-access"
        );

        // Logout of the configured account leaves the built-in entry intact.
        clear_tokens_route(home, &configured).unwrap();
        assert!(read_tokens_route(home, &configured).unwrap().is_none());
        assert_eq!(
            read_tokens(home).unwrap().unwrap().access_token,
            "builtin-access"
        );

        clear_tokens(home).unwrap();
        assert!(read_tokens(home).unwrap().is_none());
    }

    #[test]
    fn callback_rejects_wrong_state() {
        let expected = random_state();
        assert!(callback_state_matches(Some(&expected), &expected));
        // A callback carrying another login's state can never complete this
        // login; the CSRF state is single-use per login.
        assert!(!callback_state_matches(
            Some("other-login-state"),
            &expected
        ));
        assert!(!callback_state_matches(None, &expected));
    }

    #[tokio::test]
    async fn callback_port_lock_serializes_interactive_logins() {
        let first = acquire_callback_port().await;
        // A second interactive login must wait on the single Codex redirect
        // port instead of binding it concurrently.
        let mut waiter = tokio::spawn(acquire_callback_port());
        let outcome = tokio::time::timeout(Duration::from_millis(100), &mut waiter).await;
        assert!(
            outcome.is_err(),
            "second interactive login must wait for the callback port"
        );
        drop(first);
        let outcome = tokio::time::timeout(Duration::from_millis(500), waiter).await;
        assert!(
            outcome.is_ok(),
            "waiting login proceeds once the port is free"
        );
    }

    #[tokio::test]
    #[serial_test::serial(chatgpt_oauth_test_globals)]
    async fn refresh_locks_independent_per_account() {
        let lock_a = refresh_lock_for_scope("provider::alpha::oauth").await;
        let guard_a = lock_a.lock().await;

        // Same scope: a second acquire must wait (serialized).
        let lock_a2 = refresh_lock_for_scope("provider::alpha::oauth").await;
        let outcome = tokio::time::timeout(Duration::from_millis(100), lock_a2.lock()).await;
        assert!(outcome.is_err(), "same-account refresh must be serialized");
        drop(guard_a);

        // Different account: acquirable while alpha is free now.
        let lock_b = refresh_lock_for_scope("provider::beta::oauth").await;
        let guard_b = tokio::time::timeout(Duration::from_millis(100), lock_b.lock()).await;
        assert!(
            guard_b.is_ok(),
            "different accounts must not share a refresh lock"
        );
        drop(guard_b);
    }

    #[tokio::test]
    #[serial_test::serial(chatgpt_oauth_test_globals)]
    async fn refresh_locks_serialize_same_account() {
        let lock = refresh_lock_for_scope("provider::same::oauth").await;
        let lock2 = refresh_lock_for_scope("provider::same::oauth").await;
        assert!(
            Arc::ptr_eq(&lock, &lock2),
            "same scope must share one refresh lock"
        );

        let guard = lock.lock().await;
        let mut waiter = tokio::spawn(async move {
            let _g = lock2.lock().await;
            "acquired"
        });
        let outcome = tokio::time::timeout(Duration::from_millis(100), &mut waiter).await;
        assert!(
            outcome.is_err(),
            "second task must wait while the same-account lock is held"
        );

        drop(guard);
        let outcome = tokio::time::timeout(Duration::from_millis(500), waiter).await;
        assert_eq!(outcome.unwrap().unwrap(), "acquired");
    }

    async fn refresh_lock_live_scopes() -> usize {
        let locks = REFRESH_LOCKS.lock().await;
        locks.values().filter(|w| w.upgrade().is_some()).count()
    }

    async fn refresh_lock_map_has_live(scope: &str) -> bool {
        let locks = REFRESH_LOCKS.lock().await;
        locks.get(scope).and_then(|w| w.upgrade()).is_some()
    }

    async fn refresh_lock_map_contains_key(scope: &str) -> bool {
        REFRESH_LOCKS.lock().await.contains_key(scope)
    }

    #[tokio::test]
    #[serial_test::serial(chatgpt_oauth_test_globals)]
    async fn refresh_lock_map_prunes_dead_scopes() {
        let scope = "provider::churned::oauth";
        let lock = refresh_lock_for_scope(scope).await;
        assert_eq!(refresh_lock_live_scopes().await, 1);
        drop(lock);
        // No holder left: the weak entry is dead and gets pruned on the next
        // insertion, so the map stays bounded instead of growing forever.
        assert_eq!(refresh_lock_live_scopes().await, 0);
        let lock2 = refresh_lock_for_scope(scope).await;
        let lock3 = refresh_lock_for_scope(scope).await;
        assert!(
            Arc::ptr_eq(&lock2, &lock3),
            "a live lock is shared until all holders drop"
        );
        assert_eq!(refresh_lock_live_scopes().await, 1);
    }

    /// Prune must drop a *different* dead scope. Membership, not only
    /// liveness, is asserted so removing `retain` fails the test.
    #[tokio::test]
    #[serial_test::serial(chatgpt_oauth_test_globals)]
    async fn refresh_lock_map_prunes_different_dead_scope() {
        let scope_a = "provider::prune-a::oauth";
        let scope_b = "provider::prune-b::oauth";
        let lock_a = refresh_lock_for_scope(scope_a).await;
        assert!(refresh_lock_map_contains_key(scope_a).await);
        assert!(refresh_lock_map_has_live(scope_a).await);
        drop(lock_a);
        // Dead Weak remains until the next acquire prunes it.
        assert!(
            refresh_lock_map_contains_key(scope_a).await,
            "dead key A must still be present before acquire(B)"
        );
        assert!(!refresh_lock_map_has_live(scope_a).await);
        let lock_b = refresh_lock_for_scope(scope_b).await;
        assert!(
            !refresh_lock_map_contains_key(scope_a).await,
            "dead scope A must be pruned when acquiring B"
        );
        assert!(refresh_lock_map_has_live(scope_b).await);
        drop(lock_b);
    }

    /// Production path: two concurrent `valid_access_token_route` calls for
    /// the same near-expiry route must enter IdP refresh exactly once.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[serial_test::serial(chatgpt_oauth_test_globals)]
    async fn valid_access_token_route_serializes_same_account_refresh() {
        let stub = TestRefreshGuard::install(Duration::from_millis(150));
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        let route = ChatGptOAuthRoute::BuiltIn;
        let mut tokens = route_tokens("near-expiry");
        tokens.expires_at = Utc::now() - ChronoDuration::seconds(1);
        store_tokens_route(&home, &route, &tokens).unwrap();

        let home_a = home.clone();
        let home_b = home.clone();
        let route_a = route.clone();
        let route_b = route.clone();
        let t1 = tokio::spawn(async move { valid_access_token_route(&home_a, &route_a).await });
        let t2 = tokio::spawn(async move { valid_access_token_route(&home_b, &route_b).await });
        let (r1, r2) = tokio::join!(t1, t2);
        let entries = stub.entries();
        drop(stub);

        let a = r1.unwrap().unwrap().unwrap();
        let b = r2.unwrap().unwrap().unwrap();
        assert_eq!(
            entries, 1,
            "same-account concurrent refresh must hit the IdP once"
        );
        assert!(
            a.0.starts_with("refreshed-") || a.0 == b.0,
            "leader returns refreshed access token"
        );
        assert_eq!(a.0, b.0, "both callers must converge on one access token");
        let stored = read_tokens_route(&home, &route).unwrap().unwrap();
        assert!(stored.refresh_token.starts_with("rotated-"));
    }

    /// Sibling accounts may refresh concurrently (independent locks).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[serial_test::serial(chatgpt_oauth_test_globals)]
    async fn valid_access_token_route_allows_sibling_refresh() {
        let stub = TestRefreshGuard::install(Duration::from_millis(80));
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        let route_a = configured_route("sibling-a");
        let route_b = configured_route("sibling-b");
        let mut tokens_a = route_tokens("a-near");
        let mut tokens_b = route_tokens("b-near");
        tokens_a.expires_at = Utc::now() - ChronoDuration::seconds(1);
        tokens_b.expires_at = Utc::now() - ChronoDuration::seconds(1);
        store_tokens_route(&home, &route_a, &tokens_a).unwrap();
        store_tokens_route(&home, &route_b, &tokens_b).unwrap();

        let home_a = home.clone();
        let home_b = home.clone();
        let ra = route_a.clone();
        let rb = route_b.clone();
        let t1 = tokio::spawn(async move { valid_access_token_route(&home_a, &ra).await });
        let t2 = tokio::spawn(async move { valid_access_token_route(&home_b, &rb).await });
        let (r1, r2) = tokio::join!(t1, t2);
        let entries = stub.entries();
        drop(stub);

        r1.unwrap().unwrap().unwrap();
        r2.unwrap().unwrap().unwrap();
        assert_eq!(
            entries, 2,
            "distinct accounts must each refresh under their own lock"
        );
    }

    /// A just-rotated refresh token must survive transient auth-file lock
    /// contention instead of being dropped on the first WouldBlock.
    #[tokio::test]
    async fn post_refresh_store_retries_auth_file_contention() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let route = ChatGptOAuthRoute::BuiltIn;
        store_tokens_route(home, &route, &route_tokens("before")).unwrap();
        let path = home.join("auth.json");
        let held = crate::auth::manager::lock::try_lock_auth_file_nonblocking(&path)
            .expect("hold auth file lock for contention");

        let home_buf = home.to_path_buf();
        let refreshed = route_tokens("after-refresh");
        let mut task = tokio::spawn(async move {
            store_tokens_route_after_refresh(&home_buf, &ChatGptOAuthRoute::BuiltIn, &refreshed)
                .await
        });
        // Still contending: store must not finish while the lock is held.
        let early = tokio::time::timeout(Duration::from_millis(80), &mut task).await;
        assert!(early.is_err(), "store must wait while auth.json is locked");
        assert_eq!(
            read_tokens_route(home, &route)
                .unwrap()
                .unwrap()
                .access_token,
            "before",
            "contended store must not have written yet"
        );

        drop(held);
        let outcome = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("store completes within bound after lock release")
            .unwrap();
        assert!(outcome.is_ok());
        assert_eq!(
            read_tokens_route(home, &route)
                .unwrap()
                .unwrap()
                .access_token,
            "after-refresh"
        );
    }

    /// Non-contention store failures must fail fast (no full timeout wait).
    #[tokio::test]
    async fn post_refresh_store_fails_fast_on_incarnation_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc_a = ProviderIncarnation::new("123e4567-e89b-12d3-a456-426614174001").unwrap();
        let inc_b = ProviderIncarnation::new("123e4567-e89b-12d3-a456-426614174002").unwrap();
        let route_a = ChatGptOAuthRoute::Configured {
            provider_id: ProviderId::new("corp").unwrap(),
            incarnation: Some(inc_a),
        };
        let route_b = ChatGptOAuthRoute::Configured {
            provider_id: ProviderId::new("corp").unwrap(),
            incarnation: Some(inc_b),
        };
        store_tokens_route(home, &route_a, &route_tokens("live")).unwrap();

        let start = tokio::time::Instant::now();
        let err = store_tokens_route_after_refresh(home, &route_b, &route_tokens("stale"))
            .await
            .unwrap_err();
        assert!(matches!(err, ChatGptOAuthError::Store));
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "InvalidData must not burn the post-refresh wait budget"
        );
        assert_eq!(
            read_tokens_route(home, &route_a)
                .unwrap()
                .unwrap()
                .access_token,
            "live"
        );
    }

    #[test]
    fn oauth_route_binding_requires_exact_incarnation() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let inc = ProviderIncarnation::new("123e4567-e89b-12d3-a456-426614174010").unwrap();
        let other = ProviderIncarnation::new("123e4567-e89b-12d3-a456-426614174011").unwrap();
        let pid = ProviderId::new("corp").unwrap();
        let route_some = ChatGptOAuthRoute::Configured {
            provider_id: pid.clone(),
            incarnation: Some(inc.clone()),
        };
        let route_none = ChatGptOAuthRoute::Configured {
            provider_id: pid.clone(),
            incarnation: None,
        };
        let route_other = ChatGptOAuthRoute::Configured {
            provider_id: pid,
            incarnation: Some(other),
        };

        store_tokens_route(home, &route_some, &route_tokens("bound")).unwrap();
        assert!(oauth_route_binding(home, &route_some).unwrap().is_some());
        assert!(
            oauth_route_binding(home, &route_none).unwrap().is_none(),
            "None route must not observe a Some(incarnation) binding"
        );
        assert!(
            oauth_route_binding(home, &route_other).unwrap().is_none(),
            "UUID mismatch must return None"
        );
    }
}
