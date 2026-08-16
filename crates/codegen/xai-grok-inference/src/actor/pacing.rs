//! Shared inference pacing for burst-sensitive providers.
//!
//! A tool-heavy turn can submit a fresh model request after every tool result.
//! OpenRouter may therefore see several large requests in a few seconds even
//! though the shell executes only one user turn. This gate spaces normal
//! OpenRouter traffic and enters a more conservative recovery mode after a
//! 429, while leaving every other provider unchanged.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::config::InferenceConfig;
use crate::route_context::ProviderRouteContext;

const DEFAULT_MIN_INTERVAL_MS: u64 = 2_000;
const DEFAULT_RECOVERY_REQUESTS: u32 = 8;
const MIN_RECOVERY_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RECOVERY_INTERVAL: Duration = Duration::from_secs(5);
const RECOVERY_BACKOFF_SLICES: u32 = 12;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RouteKey {
    /// Legacy partition used when no route context is supplied.
    /// Preserves historical `{base_url, model}` behavior.
    Legacy { base_url: String, model: String },
    /// Account-aware partition: instance + credential route + incarnation +
    /// origin + model/operation. Two accounts at the same URL/model never collide.
    Account {
        instance_id: String,
        credential_route: String,
        incarnation: Option<String>,
        origin: String,
        model: String,
        operation: String,
    },
}

#[derive(Debug)]
struct RouteState {
    next_allowed: Instant,
    recovery_interval: Option<Duration>,
    recovery_requests_remaining: u32,
}

impl RouteState {
    fn new(now: Instant) -> Self {
        Self {
            next_allowed: now,
            recovery_interval: None,
            recovery_requests_remaining: 0,
        }
    }

    fn interval(&self, minimum: Duration) -> Duration {
        self.recovery_interval.unwrap_or(minimum).max(minimum)
    }
}

/// Process-local pacing shared by every sampler actor and subagent.
pub(crate) struct InferencePacer {
    routes: Mutex<HashMap<RouteKey, RouteState>>,
    minimum_interval: Duration,
    recovery_requests: u32,
}

impl Default for InferencePacer {
    fn default() -> Self {
        Self::from_env()
    }
}

impl InferencePacer {
    pub(crate) fn shared() -> Arc<Self> {
        static PACER: OnceLock<Arc<InferencePacer>> = OnceLock::new();
        Arc::clone(PACER.get_or_init(|| Arc::new(Self::from_env())))
    }

    pub(crate) fn from_env() -> Self {
        Self::new(
            duration_from_env(
                "GROK_OPENROUTER_MIN_REQUEST_INTERVAL_MS",
                DEFAULT_MIN_INTERVAL_MS,
            ),
            u32_from_env(
                "GROK_OPENROUTER_RATE_LIMIT_RECOVERY_REQUESTS",
                DEFAULT_RECOVERY_REQUESTS,
            ),
        )
    }

    fn new(minimum_interval: Duration, recovery_requests: u32) -> Self {
        Self {
            routes: Mutex::new(HashMap::new()),
            minimum_interval,
            recovery_requests,
        }
    }

    /// Wait until this OpenRouter model route owns the next request slot.
    ///
    /// Returns `false` when the caller cancelled while waiting. Non-OpenRouter
    /// configurations pass through immediately.
    pub(crate) async fn wait_for_slot(
        &self,
        config: &InferenceConfig,
        route: Option<&ProviderRouteContext>,
        cancel_token: &CancellationToken,
    ) -> bool {
        let Some(key) = route_key(config, route) else {
            return true;
        };
        let minimum = minimum_interval_for(self.minimum_interval, route);

        loop {
            let delay = {
                let mut routes = self.routes.lock().await;
                let now = Instant::now();
                let state = routes
                    .entry(key.clone())
                    .or_insert_with(|| RouteState::new(now));
                if state.next_allowed <= now {
                    state.next_allowed = now + state.interval(minimum);
                    None
                } else {
                    Some(state.next_allowed.duration_since(now))
                }
            };

            let Some(delay) = delay else {
                return true;
            };
            tokio::select! {
                biased;
                _ = cancel_token.cancelled() => return false,
                _ = tokio::time::sleep(delay) => {}
            }
        }
    }

    /// Apply a route-wide cooldown and conservative spacing after a 429.
    ///
    /// `backoff` is the delay the retry loop will sleep before the next
    /// attempt (from `Retry-After`, `x-ratelimit-reset`, or jitter).
    /// `server_reset`, when present, is the server-reported reset window
    /// parsed from `x-ratelimit-reset` — it overrides the jitter-derived
    /// `backoff / 12` estimate so `next_allowed` and the recovery interval
    /// track the actual window the upstream reported.
    pub(crate) async fn note_rate_limit(
        &self,
        config: &InferenceConfig,
        route: Option<&ProviderRouteContext>,
        backoff: Duration,
        server_reset: Option<Duration>,
    ) {
        let Some(key) = route_key(config, route) else {
            return;
        };
        let now = Instant::now();
        // Prefer the server-derived reset window for both the cooldown
        // and the recovery interval so the pacer tracks the actual limit
        // instead of the backoff-guess slice (backoff / 12).
        let cooldown = server_reset.unwrap_or(backoff);
        let recovery_interval = (cooldown / RECOVERY_BACKOFF_SLICES)
            .clamp(MIN_RECOVERY_INTERVAL, MAX_RECOVERY_INTERVAL);
        let recovery_requests = recovery_requests_for(self.recovery_requests, route);
        let recovery_interval_ms = if recovery_requests > 0 {
            recovery_interval.as_millis() as u64
        } else {
            0
        };
        let mut routes = self.routes.lock().await;
        let state = routes.entry(key).or_insert_with(|| RouteState::new(now));
        state.next_allowed = state.next_allowed.max(now + cooldown);
        state.recovery_interval = (recovery_requests > 0).then(|| {
            state
                .recovery_interval
                .unwrap_or_default()
                .max(recovery_interval)
        });
        state.recovery_requests_remaining = recovery_requests;
        tracing::warn!(
            target: crate::inference_log::TARGET,
            model = %config.model,
            backoff_ms = backoff.as_millis() as u64,
            server_reset_ms = server_reset.map(|d| d.as_millis() as u64),
            recovery_interval_ms,
            recovery_requests,
            "OpenRouter rate-limit pacing enabled"
        );
    }

    /// Count a successful request toward leaving conservative recovery mode.
    pub(crate) async fn note_success(
        &self,
        config: &InferenceConfig,
        route: Option<&ProviderRouteContext>,
    ) {
        let Some(key) = route_key(config, route) else {
            return;
        };
        let mut routes = self.routes.lock().await;
        let Some(state) = routes.get_mut(&key) else {
            return;
        };
        if state.recovery_requests_remaining == 0 {
            return;
        }
        state.recovery_requests_remaining -= 1;
        if state.recovery_requests_remaining == 0 {
            state.recovery_interval = None;
            let minimum = minimum_interval_for(self.minimum_interval, route);
            state.next_allowed = state.next_allowed.min(Instant::now() + minimum);
            tracing::info!(
                target: crate::inference_log::TARGET,
                model = %config.model,
                "OpenRouter rate-limit pacing returned to normal"
            );
        }
    }
}

/// Whether OpenRouter request pacing applies to this config / route.
///
/// Precedence:
/// 1. Explicit route pacing override (`enabled: Some(true/false)`).
/// 2. [`ProviderIdentity::OpenRouter`] — always paces (identity gate).
/// 3. Explicit [`InferenceConfig::openrouter_pacing`] — opt-in for
///    OpenRouter-compatible proxies that keep a different identity.
/// 4. Legacy hostname match on `openrouter.ai` — transitional fallback when
///    identity was not propagated; does **not** enable OpenRouter request
///    extensions (provider/plugins/reasoning), only spacing.
pub(crate) fn openrouter_pacing_applies(
    config: &InferenceConfig,
    route: Option<&ProviderRouteContext>,
) -> bool {
    if let Some(enabled) = route.and_then(|r| r.pacing().enabled) {
        return enabled;
    }
    if config.provider_identity.is_openrouter() {
        return true;
    }
    if config.openrouter_pacing {
        return true;
    }
    host_is_openrouter(&config.base_url)
}

fn host_is_openrouter(base_url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(base_url) else {
        return false;
    };
    // Never trust userinfo-bearing authority for host matching.
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    host == "openrouter.ai" || host.ends_with(".openrouter.ai")
}

fn route_key(config: &InferenceConfig, route: Option<&ProviderRouteContext>) -> Option<RouteKey> {
    if !openrouter_pacing_applies(config, route) {
        return None;
    }
    if let Some(route) = route {
        let (instance_id, credential_route, incarnation, origin, model, operation) =
            route.pacing_partition(&config.model);
        return Some(RouteKey::Account {
            instance_id,
            credential_route,
            incarnation,
            origin,
            model,
            operation,
        });
    }
    Some(RouteKey::Legacy {
        base_url: config.base_url.clone(),
        model: config.model.clone(),
    })
}

fn minimum_interval_for(default: Duration, route: Option<&ProviderRouteContext>) -> Duration {
    route
        .and_then(|r| r.pacing().min_interval_ms)
        .map(Duration::from_millis)
        .unwrap_or(default)
}

fn recovery_requests_for(default: u32, route: Option<&ProviderRouteContext>) -> u32 {
    // RoutePacingOverride currently exposes enabled/min_interval only;
    // recovery_requests fall through to process-wide default.
    let _ = route;
    default
}

fn duration_from_env(name: &str, default_ms: u64) -> Duration {
    let milliseconds = std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default_ms);
    Duration::from_millis(milliseconds)
}

fn u32_from_env(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ApiBackend;
    use crate::config::InferenceConfig;

    fn config(base_url: &str) -> InferenceConfig {
        InferenceConfig {
            base_url: base_url.to_owned(),
            model: "openai/gpt-oss-120b".to_owned(),
            api_backend: ApiBackend::ChatCompletions,
            ..InferenceConfig::default()
        }
    }

    fn openrouter_identity_config(base_url: &str) -> InferenceConfig {
        InferenceConfig {
            base_url: base_url.to_owned(),
            model: "openai/gpt-oss-120b".to_owned(),
            api_backend: ApiBackend::ChatCompletions,
            provider_identity: crate::config::ProviderIdentity::OpenRouter,
            ..InferenceConfig::default()
        }
    }

    #[tokio::test(start_paused = true)]
    async fn non_openrouter_routes_are_not_delayed() {
        let pacer = InferencePacer::new(Duration::from_secs(30), 8);
        let cancel = CancellationToken::new();
        assert!(
            pacer
                .wait_for_slot(&config("https://api.openai.com/v1"), None, &cancel)
                .await
        );
    }

    #[tokio::test(start_paused = true)]
    async fn openrouter_requests_are_spaced() {
        let pacer = InferencePacer::new(Duration::from_secs(2), 8);
        // Host fallback still applies when identity is Custom.
        let config = config("https://openrouter.ai/api/v1");
        let cancel = CancellationToken::new();

        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        let started = Instant::now();
        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        assert_eq!(
            Instant::now().duration_since(started),
            Duration::from_secs(2)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn openrouter_identity_paces_even_on_proxy_host() {
        let pacer = InferencePacer::new(Duration::from_secs(2), 8);
        let config = openrouter_identity_config("https://or-proxy.example/api/v1");
        let cancel = CancellationToken::new();
        assert!(openrouter_pacing_applies(&config, None));
        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        let started = Instant::now();
        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        assert_eq!(
            Instant::now().duration_since(started),
            Duration::from_secs(2)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn explicit_pacing_flag_opts_in_custom_proxy() {
        let pacer = InferencePacer::new(Duration::from_secs(2), 8);
        let mut config = config("https://custom-proxy.example/v1");
        assert!(!openrouter_pacing_applies(&config, None));
        config.openrouter_pacing = true;
        assert!(openrouter_pacing_applies(&config, None));
        let cancel = CancellationToken::new();
        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        let started = Instant::now();
        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        assert_eq!(
            Instant::now().duration_since(started),
            Duration::from_secs(2)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limit_enables_conservative_recovery_spacing() {
        let pacer = InferencePacer::new(Duration::from_millis(100), 2);
        let config = config("https://openrouter.ai/api/v1");
        let cancel = CancellationToken::new();

        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        pacer
            .note_rate_limit(&config, None, Duration::from_secs(60), None)
            .await;

        let cooldown_started = Instant::now();
        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        assert_eq!(
            Instant::now().duration_since(cooldown_started),
            Duration::from_secs(60)
        );
        pacer.note_success(&config, None).await;

        let recovery_started = Instant::now();
        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        assert_eq!(
            Instant::now().duration_since(recovery_started),
            Duration::from_secs(5)
        );
        pacer.note_success(&config, None).await;

        let normal_started = Instant::now();
        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        assert_eq!(
            Instant::now().duration_since(normal_started),
            Duration::from_millis(100)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn zero_recovery_requests_returns_to_normal_after_cooldown() {
        let pacer = InferencePacer::new(Duration::from_millis(100), 0);
        let config = config("https://openrouter.ai/api/v1");
        let cancel = CancellationToken::new();

        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        pacer
            .note_rate_limit(&config, None, Duration::from_secs(60), None)
            .await;

        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        let normal_started = Instant::now();
        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        assert_eq!(
            Instant::now().duration_since(normal_started),
            Duration::from_millis(100)
        );
    }

    /// When the server reports a reset window, `note_rate_limit` uses it
    /// for both the cooldown and the recovery interval instead of the
    /// jitter-derived `backoff / 12` guess. Here a short reset (12s) with
    /// a large backoff (60s) must produce a 12s cooldown (reset wins),
    /// and the recovery interval must derive from 12s/12 = 1s.
    #[tokio::test(start_paused = true)]
    async fn rate_limit_uses_server_reset_over_backoff_for_cooldown() {
        let pacer = InferencePacer::new(Duration::from_millis(100), 2);
        let config = config("https://openrouter.ai/api/v1");
        let cancel = CancellationToken::new();

        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        // backoff=60s (jitter guess) but server_reset=12s wins.
        pacer
            .note_rate_limit(&config, None,
                Duration::from_secs(60),
                Some(Duration::from_secs(12)),
            )
            .await;

        let cooldown_started = Instant::now();
        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        // Cooldown tracks the server reset (12s), not the backoff (60s).
        assert_eq!(
            Instant::now().duration_since(cooldown_started),
            Duration::from_secs(12)
        );
        pacer.note_success(&config, None).await;

        // Recovery interval = 12s / 12 = 1s (clamped to MIN_RECOVERY_INTERVAL).
        let recovery_started = Instant::now();
        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        assert_eq!(
            Instant::now().duration_since(recovery_started),
            Duration::from_secs(1)
        );
    }

    /// Without a server reset, the pacer falls back to the backoff for the
    /// cooldown and derives the recovery interval from backoff / 12 (clamped).
    #[tokio::test(start_paused = true)]
    async fn rate_limit_without_reset_uses_backoff_for_cooldown() {
        let pacer = InferencePacer::new(Duration::from_millis(100), 2);
        let config = config("https://openrouter.ai/api/v1");
        let cancel = CancellationToken::new();

        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        pacer
            .note_rate_limit(&config, None, Duration::from_secs(60), None)
            .await;

        let cooldown_started = Instant::now();
        assert!(pacer.wait_for_slot(&config, None, &cancel).await);
        assert_eq!(
            Instant::now().duration_since(cooldown_started),
            Duration::from_secs(60)
        );
    }
}
