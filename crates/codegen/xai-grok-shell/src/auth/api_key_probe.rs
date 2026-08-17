//! Selective first-party environment API-key probe.
//!
//! Before `initialize` lets an `XAI_API_KEY`-only process suppress the normal
//! interactive login path, probe `GET {xai_api_base_url}/api-key` against the
//! configured first-party endpoint. Provider-scoped BYOK credentials are never
//! sent to this endpoint and never cause this probe to run.
//!
//! A known usable response keeps API-key advertisement; a known unusable
//! response suppresses it. Timeout, transport failure, and response-shape drift
//! fail open so an unavailable probe endpoint cannot falsely block startup.

use std::time::{Duration, Instant};

use serde::Deserialize;

/// Wall-clock budget for the entire probe, including the single retry.
pub(crate) const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_millis(400);

/// Inputs that determine whether the first-party environment key is the only
/// reason normal login would be suppressed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FirstPartyEnvProbePolicy {
    pub disable_api_key_auth: bool,
    pub has_alternative_api_key_route: bool,
    pub has_first_party_env_key: bool,
    pub has_usable_session: bool,
    pub preferred_method_is_set: bool,
}

/// Probe only when a first-party xAI environment key alone would suppress
/// normal login. An already usable session independently determines startup
/// auth and therefore skips the probe.
pub(crate) fn should_probe_first_party_env_key(policy: FirstPartyEnvProbePolicy) -> bool {
    !policy.disable_api_key_auth
        && !policy.has_alternative_api_key_route
        && policy.has_first_party_env_key
        && !policy.has_usable_session
        && !policy.preferred_method_is_set
}

const MAX_RETRIES: u32 = 1;
const RETRY_BACKOFF: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApiKeyProbeVerdict {
    Usable,
    Unusable,
    Unknown,
}

impl ApiKeyProbeVerdict {
    pub(crate) fn allows_advertise(self) -> bool {
        !matches!(self, Self::Unusable)
    }
}

#[derive(Debug, Default, Deserialize)]
struct ApiKeyInfoBody {
    api_key_id: Option<serde_json::Value>,
    api_key_blocked: Option<bool>,
    api_key_disabled: Option<bool>,
    team_blocked: Option<bool>,
}

impl ApiKeyInfoBody {
    fn verdict(&self) -> Option<ApiKeyProbeVerdict> {
        if self.api_key_blocked == Some(true)
            || self.api_key_disabled == Some(true)
            || self.team_blocked == Some(true)
        {
            return Some(ApiKeyProbeVerdict::Unusable);
        }
        (self.api_key_id.is_some()
            || self.api_key_blocked.is_some()
            || self.api_key_disabled.is_some()
            || self.team_blocked.is_some())
        .then_some(ApiKeyProbeVerdict::Usable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttemptOutcome {
    Done(ApiKeyProbeVerdict),
    Retry,
}

fn api_key_info_url(api_base_url: &str) -> String {
    let base = api_base_url.trim().trim_end_matches('/');
    format!("{base}/api-key")
}

fn normalize_error_marker(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn known_unusable_error_marker(value: &str) -> bool {
    let marker = normalize_error_marker(value);
    matches!(
        marker.as_str(),
        "invalid api key"
            | "incorrect api key"
            | "api key is invalid"
            | "api key blocked"
            | "api key is blocked"
            | "api key disabled"
            | "api key is disabled"
            | "team blocked"
            | "team is blocked"
    )
}

fn body_has_known_unusable_error(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    let candidates = [
        value.get("code").and_then(serde_json::Value::as_str),
        value.get("error").and_then(serde_json::Value::as_str),
        value.get("message").and_then(serde_json::Value::as_str),
        value
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(serde_json::Value::as_str),
        value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str),
    ];
    candidates
        .into_iter()
        .flatten()
        .any(known_unusable_error_marker)
}

fn retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

/// Classify only responses whose meaning is explicit. In particular, a generic
/// 401 is unknown: status alone does not establish which credential an
/// intermediary or endpoint rejected.
fn classify_probe_attempt(status: u16, body: &[u8]) -> AttemptOutcome {
    if status == 200 {
        let verdict = serde_json::from_slice::<ApiKeyInfoBody>(body)
            .ok()
            .and_then(|info| info.verdict())
            .unwrap_or(ApiKeyProbeVerdict::Unknown);
        return AttemptOutcome::Done(verdict);
    }

    if matches!(status, 400..=403) && body_has_known_unusable_error(body) {
        return AttemptOutcome::Done(ApiKeyProbeVerdict::Unusable);
    }

    if retryable_status(status) {
        AttemptOutcome::Retry
    } else {
        AttemptOutcome::Done(ApiKeyProbeVerdict::Unknown)
    }
}

#[cfg(test)]
fn classify_probe_response(status: u16, body: &[u8]) -> ApiKeyProbeVerdict {
    match classify_probe_attempt(status, body) {
        AttemptOutcome::Done(verdict) => verdict,
        AttemptOutcome::Retry => ApiKeyProbeVerdict::Unknown,
    }
}

fn retryable_transport_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

async fn probe_xai_api_key(key: &str, api_base_url: &str, timeout: Duration) -> ApiKeyProbeVerdict {
    probe_xai_api_key_at_url(key, &api_key_info_url(api_base_url), timeout).await
}

async fn probe_xai_api_key_at_url(key: &str, url: &str, timeout: Duration) -> ApiKeyProbeVerdict {
    if key.trim().is_empty() {
        return ApiKeyProbeVerdict::Unusable;
    }

    let client = crate::http::shared_client();
    let started = Instant::now();
    let deadline = started + timeout;
    let mut attempts = 0u32;
    let mut verdict = ApiKeyProbeVerdict::Unknown;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        attempts += 1;

        let outcome = match client
            .get(url)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {key}"))
            .timeout(remaining)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                match response.bytes().await {
                    Ok(body) => classify_probe_attempt(status, &body),
                    Err(error) if retryable_transport_error(&error) => AttemptOutcome::Retry,
                    Err(_) => AttemptOutcome::Done(ApiKeyProbeVerdict::Unknown),
                }
            }
            Err(error) if retryable_transport_error(&error) => AttemptOutcome::Retry,
            Err(_) => AttemptOutcome::Done(ApiKeyProbeVerdict::Unknown),
        };

        match outcome {
            AttemptOutcome::Done(done) => {
                verdict = done;
                break;
            }
            AttemptOutcome::Retry if attempts <= MAX_RETRIES => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                tokio::time::sleep(RETRY_BACKOFF.min(remaining)).await;
            }
            AttemptOutcome::Retry => break,
        }
    }

    xai_grok_telemetry::unified_log::info(
        "auth: first-party API key probe",
        None,
        Some(serde_json::json!({
            "verdict": format!("{verdict:?}"),
            "allows_advertise": verdict.allows_advertise(),
            "elapsed_ms": started.elapsed().as_millis() as u64,
            "timeout_ms": timeout.as_millis() as u64,
            "attempts": attempts,
        })),
    );

    verdict
}

/// Probe the first-party xAI environment key, failing open on unknown results.
pub(crate) async fn first_party_env_key_allows_advertise(
    api_base_url: &str,
    timeout: Duration,
) -> bool {
    let Ok(key) = crate::agent::auth_method::read_xai_api_key_env() else {
        return false;
    };
    probe_xai_api_key(&key, api_base_url, timeout)
        .await
        .allows_advertise()
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    use super::*;

    #[test]
    fn probe_policy_matrix() {
        let cases = [
            (
                "first-party env key alone",
                FirstPartyEnvProbePolicy {
                    disable_api_key_auth: false,
                    has_alternative_api_key_route: false,
                    has_first_party_env_key: true,
                    has_usable_session: false,
                    preferred_method_is_set: false,
                },
                true,
            ),
            (
                "api-key auth disabled",
                FirstPartyEnvProbePolicy {
                    disable_api_key_auth: true,
                    has_alternative_api_key_route: false,
                    has_first_party_env_key: true,
                    has_usable_session: false,
                    preferred_method_is_set: false,
                },
                false,
            ),
            (
                "provider-scoped BYOK present",
                FirstPartyEnvProbePolicy {
                    disable_api_key_auth: false,
                    has_alternative_api_key_route: true,
                    has_first_party_env_key: true,
                    has_usable_session: false,
                    preferred_method_is_set: false,
                },
                false,
            ),
            (
                "no first-party env key",
                FirstPartyEnvProbePolicy {
                    disable_api_key_auth: false,
                    has_alternative_api_key_route: false,
                    has_first_party_env_key: false,
                    has_usable_session: false,
                    preferred_method_is_set: false,
                },
                false,
            ),
            (
                "usable session present",
                FirstPartyEnvProbePolicy {
                    disable_api_key_auth: false,
                    has_alternative_api_key_route: false,
                    has_first_party_env_key: true,
                    has_usable_session: true,
                    preferred_method_is_set: false,
                },
                false,
            ),
            (
                "preferred method set",
                FirstPartyEnvProbePolicy {
                    disable_api_key_auth: false,
                    has_alternative_api_key_route: false,
                    has_first_party_env_key: true,
                    has_usable_session: false,
                    preferred_method_is_set: true,
                },
                false,
            ),
        ];

        for (name, policy, expected) in cases {
            assert_eq!(should_probe_first_party_env_key(policy), expected, "{name}");
        }
    }

    #[test]
    fn joins_api_key_path_onto_configured_base() {
        assert_eq!(
            api_key_info_url("https://api.x.ai/v1"),
            "https://api.x.ai/v1/api-key"
        );
        assert_eq!(
            api_key_info_url(" https://enterprise.example/v1/ "),
            "https://enterprise.example/v1/api-key"
        );
    }

    #[test]
    fn response_classification_matrix() {
        let cases: &[(u16, &[u8], ApiKeyProbeVerdict)] = &[
            (
                200,
                br#"{"api_key_id":"key-1"}"#,
                ApiKeyProbeVerdict::Usable,
            ),
            (
                200,
                br#"{"api_key_blocked":false,"api_key_disabled":false,"team_blocked":false}"#,
                ApiKeyProbeVerdict::Usable,
            ),
            (
                200,
                br#"{"api_key_blocked":true}"#,
                ApiKeyProbeVerdict::Unusable,
            ),
            (
                200,
                br#"{"api_key_disabled":true}"#,
                ApiKeyProbeVerdict::Unusable,
            ),
            (
                200,
                br#"{"team_blocked":true}"#,
                ApiKeyProbeVerdict::Unusable,
            ),
            (200, b"not-json", ApiKeyProbeVerdict::Unknown),
            (
                400,
                br#"{"error":"Incorrect API key"}"#,
                ApiKeyProbeVerdict::Unusable,
            ),
            (
                401,
                br#"{"error":{"code":"invalid_api_key"}}"#,
                ApiKeyProbeVerdict::Unusable,
            ),
            (
                401,
                br#"{"error":"unauthorized"}"#,
                ApiKeyProbeVerdict::Unknown,
            ),
            (401, b"", ApiKeyProbeVerdict::Unknown),
            (404, b"", ApiKeyProbeVerdict::Unknown),
            (429, b"", ApiKeyProbeVerdict::Unknown),
            (503, b"", ApiKeyProbeVerdict::Unknown),
        ];

        for (status, body, expected) in cases {
            assert_eq!(
                classify_probe_response(*status, body),
                *expected,
                "status={status}, body={}",
                String::from_utf8_lossy(body)
            );
        }
    }

    #[test]
    fn only_approved_transient_statuses_retry() {
        for status in [429, 500, 502, 503, 504] {
            assert_eq!(classify_probe_attempt(status, b""), AttemptOutcome::Retry);
        }
        for status in [401, 403, 404, 408, 501, 505] {
            assert_ne!(classify_probe_attempt(status, b""), AttemptOutcome::Retry);
        }
    }

    struct MockResponse {
        status: &'static str,
        body: &'static [u8],
        stall: Option<Duration>,
    }

    fn serve_sequence(responses: Vec<MockResponse>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let address = listener.local_addr().expect("mock server address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        std::thread::spawn(move || {
            for response in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut request = [0u8; 4096];
                let read = stream.read(&mut request).unwrap_or(0);
                recorded
                    .lock()
                    .expect("record requests")
                    .push(String::from_utf8_lossy(&request[..read]).into_owned());
                if let Some(stall) = response.stall {
                    std::thread::sleep(stall);
                }
                let wire = format!(
                    "{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response.status,
                    response.body.len(),
                    String::from_utf8_lossy(response.body)
                );
                let _ = stream.write_all(wire.as_bytes());
            }
        });
        (format!("http://{address}/v1"), requests)
    }

    #[tokio::test]
    async fn local_http_probe_uses_configured_path_and_first_party_bearer() {
        let (base, requests) = serve_sequence(vec![MockResponse {
            status: "HTTP/1.1 200 OK",
            body: br#"{"api_key_blocked":false,"api_key_disabled":false}"#,
            stall: None,
        }]);

        assert_eq!(
            probe_xai_api_key("xai-local-probe", &base, Duration::from_secs(2)).await,
            ApiKeyProbeVerdict::Usable
        );
        let request = requests.lock().expect("requests").join("\n");
        assert!(request.starts_with("GET /v1/api-key HTTP/1.1"), "{request}");
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer xai-local-probe"),
            "{request}"
        );
    }

    #[tokio::test]
    async fn local_http_known_invalid_key_is_unusable() {
        let (base, _) = serve_sequence(vec![MockResponse {
            status: "HTTP/1.1 400 Bad Request",
            body: br#"{"error":"Incorrect API key"}"#,
            stall: None,
        }]);
        assert_eq!(
            probe_xai_api_key("xai-invalid", &base, Duration::from_secs(2)).await,
            ApiKeyProbeVerdict::Unusable
        );
    }

    #[tokio::test]
    async fn local_http_generic_401_is_unknown_and_fails_open() {
        let (base, _) = serve_sequence(vec![MockResponse {
            status: "HTTP/1.1 401 Unauthorized",
            body: br#"{"error":"unauthorized"}"#,
            stall: None,
        }]);
        let verdict = probe_xai_api_key("xai-unknown", &base, Duration::from_secs(2)).await;
        assert_eq!(verdict, ApiKeyProbeVerdict::Unknown);
        assert!(verdict.allows_advertise());
    }

    #[tokio::test]
    async fn local_http_retries_approved_transient_once() {
        let (base, requests) = serve_sequence(vec![
            MockResponse {
                status: "HTTP/1.1 503 Service Unavailable",
                body: b"",
                stall: None,
            },
            MockResponse {
                status: "HTTP/1.1 200 OK",
                body: br#"{"api_key_blocked":false}"#,
                stall: None,
            },
        ]);
        assert_eq!(
            probe_xai_api_key("xai-retry", &base, Duration::from_secs(2)).await,
            ApiKeyProbeVerdict::Usable
        );
        assert_eq!(requests.lock().expect("requests").len(), 2);
    }

    #[tokio::test]
    async fn local_http_timeout_is_unknown_and_fails_open() {
        let (base, _) = serve_sequence(vec![MockResponse {
            status: "HTTP/1.1 200 OK",
            body: br#"{"api_key_blocked":false}"#,
            stall: Some(Duration::from_secs(1)),
        }]);
        let verdict = probe_xai_api_key("xai-timeout", &base, Duration::from_millis(50)).await;
        assert_eq!(verdict, ApiKeyProbeVerdict::Unknown);
        assert!(verdict.allows_advertise());
    }
}
