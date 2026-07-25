//! Native ChatGPT / OpenAI subscription OAuth (OpenCode/Codex-compatible).
//!
//! Protocol mirrors OpenCode's `plugin/openai/codex.ts`:
//! - Issuer `https://auth.openai.com`
//! - Public Codex client id
//! - Browser authorization-code + PKCE, or headless device auth
//! - Tokens stored under [`OPENAI_OAUTH_SCOPE`] in `auth.json`
//!
//! Mutual exclusion with [`super::storage::OPENAI_API_KEY_SCOPE`]: storing OAuth
//! clears the API key and vice versa (enforced by the store helpers).

use std::path::Path;
use std::time::Duration;

use base64::Engine;
use chrono::{Duration as ChronoDuration, Utc};
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use super::model::{AuthMode, GrokAuth};
use super::storage::{
    OPENAI_API_KEY_SCOPE, OPENAI_OAUTH_SCOPE, clear_provider_api_key, read_auth_json,
    read_auth_json_or_empty, write_auth_json,
};

/// Public Codex CLI / OpenCode client id for ChatGPT subscription OAuth.
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const ISSUER: &str = "https://auth.openai.com";
/// Responses base (Grok appends `/responses`).
pub const CODEX_RESPONSES_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
/// Wire headers must match OpenCode/Codex subscription clients (not Grok-branded).
pub const ORIGINATOR: &str = "opencode";
const OAUTH_PORT: u16 = 1455;
const OAUTH_CALLBACK_PATH: &str = "/auth/callback";
const TOKEN_SKEW_SECS: i64 = 60;
const BROWSER_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const DEVICE_POLL_SAFETY_MARGIN: Duration = Duration::from_secs(3);

static REFRESH_LOCK: Mutex<()> = Mutex::const_new(());

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
    let client = reqwest::Client::new();
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

async fn refresh_with_token(refresh_token: &str) -> Result<ChatGptOAuthTokens, ChatGptOAuthError> {
    let form = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}",
        urlencoding_encode(refresh_token),
        urlencoding_encode(CLIENT_ID)
    );
    let client = reqwest::Client::new();
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

/// Persist OAuth tokens and clear any OpenAI API key (mutual exclusion).
pub fn store_tokens(
    grok_home: &Path,
    tokens: &ChatGptOAuthTokens,
) -> Result<(), ChatGptOAuthError> {
    let path = grok_home.join("auth.json");
    let lock = crate::auth::manager::lock::try_lock_auth_file_nonblocking(&path)
        .ok_or(ChatGptOAuthError::Store)?;
    if !lock.still_live(&path) {
        return Err(ChatGptOAuthError::Store);
    }
    let mut store = read_auth_json_or_empty(&path).map_err(|_| ChatGptOAuthError::Store)?;
    store.remove(OPENAI_API_KEY_SCOPE);
    store.insert(OPENAI_OAUTH_SCOPE.to_owned(), tokens_to_auth(tokens));
    if !lock.still_live(&path) {
        return Err(ChatGptOAuthError::Store);
    }
    write_auth_json(&path, &store).map_err(|_| ChatGptOAuthError::Store)
}

pub fn clear_tokens(grok_home: &Path) -> Result<(), ChatGptOAuthError> {
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

pub fn read_tokens(grok_home: &Path) -> Result<Option<ChatGptOAuthTokens>, ChatGptOAuthError> {
    let path = grok_home.join("auth.json");
    match read_auth_json(&path) {
        Ok(store) => Ok(store.get(OPENAI_OAUTH_SCOPE).and_then(auth_to_tokens)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(ChatGptOAuthError::Store),
    }
}

pub fn status(grok_home: &Path) -> ChatGptOAuthStatus {
    match read_tokens(grok_home) {
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

/// Return a usable access token, refreshing when near expiry.
pub async fn valid_access_token(
    grok_home: &Path,
) -> Result<Option<(String, Option<String>)>, ChatGptOAuthError> {
    let Some(tokens) = read_tokens(grok_home)? else {
        return Ok(None);
    };
    let near_expiry = tokens.expires_at <= Utc::now() + ChronoDuration::seconds(TOKEN_SKEW_SECS);
    if !near_expiry {
        return Ok(Some((tokens.access_token, tokens.account_id)));
    }
    let _guard = REFRESH_LOCK.lock().await;
    // Re-read after lock in case another task refreshed.
    let Some(tokens) = read_tokens(grok_home)? else {
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
    store_tokens(grok_home, &refreshed)?;
    Ok(Some((refreshed.access_token, refreshed.account_id)))
}

/// OpenAI API key store path that also clears OAuth (mutual exclusion).
pub fn store_api_key_exclusive(grok_home: &Path, api_key: &str) -> Result<(), ChatGptOAuthError> {
    // Clear OAuth first, then store key via normal helper.
    let _ = clear_tokens(grok_home);
    crate::auth::store_provider_api_key(grok_home, OPENAI_API_KEY_SCOPE, api_key)
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

/// Browser PKCE login. Binds `localhost:1455` (Codex redirect_uri).
pub async fn login_browser(grok_home: &Path) -> Result<ChatGptOAuthTokens, ChatGptOAuthError> {
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

            if got_state.as_deref() != Some(state.as_str()) {
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

    store_tokens(grok_home, &tokens)?;
    Ok(tokens)
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
    let client = reqwest::Client::new();
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

pub async fn complete_device_login(
    grok_home: &Path,
    start: &DeviceLoginStart,
) -> Result<ChatGptOAuthTokens, ChatGptOAuthError> {
    let client = reqwest::Client::new();
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
            store_tokens(grok_home, &tokens)?;
            return Ok(tokens);
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

/// Full device login: start, print the user code, open browser, poll to completion.
///
/// OpenAI's verification page requires the user to type the 9-character code
/// shown in the terminal. Always surface it on stderr (OpenCode equivalent of
/// `instructions: Enter code: …`); `tracing` alone is invisible in normal use.
pub async fn login_device(grok_home: &Path) -> Result<ChatGptOAuthTokens, ChatGptOAuthError> {
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
    complete_device_login(grok_home, &start).await
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
    fn oauth_store_clears_api_key() {
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
        assert!(
            crate::auth::read_provider_api_key(dir.path(), OPENAI_API_KEY_SCOPE)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn api_key_exclusive_clears_oauth() {
        let dir = tempfile::tempdir().unwrap();
        let tokens = ChatGptOAuthTokens {
            access_token: "access".into(),
            refresh_token: "refresh".into(),
            expires_at: Utc::now() + ChronoDuration::hours(1),
            account_id: None,
            email: None,
        };
        store_tokens(dir.path(), &tokens).unwrap();
        store_api_key_exclusive(dir.path(), "sk-new").unwrap();
        assert!(read_tokens(dir.path()).unwrap().is_none());
        assert_eq!(
            crate::auth::read_provider_api_key(dir.path(), OPENAI_API_KEY_SCOPE)
                .unwrap()
                .as_deref(),
            Some("sk-new")
        );
    }
}
