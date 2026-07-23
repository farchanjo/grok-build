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

use crate::config::SamplerConfig;

const DEFAULT_MIN_INTERVAL_MS: u64 = 2_000;
const DEFAULT_RECOVERY_REQUESTS: u32 = 8;
const MIN_RECOVERY_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RECOVERY_INTERVAL: Duration = Duration::from_secs(5);
const RECOVERY_BACKOFF_SLICES: u32 = 12;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RouteKey {
    base_url: String,
    model: String,
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
        config: &SamplerConfig,
        cancel_token: &CancellationToken,
    ) -> bool {
        let Some(key) = route_key(config) else {
            return true;
        };

        loop {
            let delay = {
                let mut routes = self.routes.lock().await;
                let now = Instant::now();
                let state = routes
                    .entry(key.clone())
                    .or_insert_with(|| RouteState::new(now));
                if state.next_allowed <= now {
                    state.next_allowed = now + state.interval(self.minimum_interval);
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
    pub(crate) async fn note_rate_limit(&self, config: &SamplerConfig, backoff: Duration) {
        let Some(key) = route_key(config) else {
            return;
        };
        let now = Instant::now();
        let recovery_interval =
            (backoff / RECOVERY_BACKOFF_SLICES).clamp(MIN_RECOVERY_INTERVAL, MAX_RECOVERY_INTERVAL);
        let recovery_interval_ms = if self.recovery_requests > 0 {
            recovery_interval.as_millis() as u64
        } else {
            0
        };
        let mut routes = self.routes.lock().await;
        let state = routes.entry(key).or_insert_with(|| RouteState::new(now));
        state.next_allowed = state.next_allowed.max(now + backoff);
        state.recovery_interval = (self.recovery_requests > 0).then(|| {
            state
                .recovery_interval
                .unwrap_or_default()
                .max(recovery_interval)
        });
        state.recovery_requests_remaining = self.recovery_requests;
        tracing::warn!(
            target: crate::sampling_log::TARGET,
            model = %config.model,
            backoff_ms = backoff.as_millis() as u64,
            recovery_interval_ms,
            recovery_requests = self.recovery_requests,
            "OpenRouter rate-limit pacing enabled"
        );
    }

    /// Count a successful request toward leaving conservative recovery mode.
    pub(crate) async fn note_success(&self, config: &SamplerConfig) {
        let Some(key) = route_key(config) else {
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
            state.next_allowed = state
                .next_allowed
                .min(Instant::now() + self.minimum_interval);
            tracing::info!(
                target: crate::sampling_log::TARGET,
                model = %config.model,
                "OpenRouter rate-limit pacing returned to normal"
            );
        }
    }
}

fn route_key(config: &SamplerConfig) -> Option<RouteKey> {
    let url = reqwest::Url::parse(&config.base_url).ok()?;
    let host = url.host_str()?;
    if host != "openrouter.ai" && !host.ends_with(".openrouter.ai") {
        return None;
    }
    Some(RouteKey {
        base_url: config.base_url.clone(),
        model: config.model.clone(),
    })
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
    use crate::config::SamplerConfig;

    fn config(base_url: &str) -> SamplerConfig {
        SamplerConfig {
            base_url: base_url.to_owned(),
            model: "openai/gpt-oss-120b".to_owned(),
            api_backend: ApiBackend::ChatCompletions,
            ..SamplerConfig::default()
        }
    }

    #[tokio::test(start_paused = true)]
    async fn non_openrouter_routes_are_not_delayed() {
        let pacer = InferencePacer::new(Duration::from_secs(30), 8);
        let cancel = CancellationToken::new();
        assert!(
            pacer
                .wait_for_slot(&config("https://api.openai.com/v1"), &cancel)
                .await
        );
    }

    #[tokio::test(start_paused = true)]
    async fn openrouter_requests_are_spaced() {
        let pacer = InferencePacer::new(Duration::from_secs(2), 8);
        let config = config("https://openrouter.ai/api/v1");
        let cancel = CancellationToken::new();

        assert!(pacer.wait_for_slot(&config, &cancel).await);
        let started = Instant::now();
        assert!(pacer.wait_for_slot(&config, &cancel).await);
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

        assert!(pacer.wait_for_slot(&config, &cancel).await);
        pacer
            .note_rate_limit(&config, Duration::from_secs(60))
            .await;

        let cooldown_started = Instant::now();
        assert!(pacer.wait_for_slot(&config, &cancel).await);
        assert_eq!(
            Instant::now().duration_since(cooldown_started),
            Duration::from_secs(60)
        );
        pacer.note_success(&config).await;

        let recovery_started = Instant::now();
        assert!(pacer.wait_for_slot(&config, &cancel).await);
        assert_eq!(
            Instant::now().duration_since(recovery_started),
            Duration::from_secs(5)
        );
        pacer.note_success(&config).await;

        let normal_started = Instant::now();
        assert!(pacer.wait_for_slot(&config, &cancel).await);
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

        assert!(pacer.wait_for_slot(&config, &cancel).await);
        pacer
            .note_rate_limit(&config, Duration::from_secs(60))
            .await;

        assert!(pacer.wait_for_slot(&config, &cancel).await);
        let normal_started = Instant::now();
        assert!(pacer.wait_for_slot(&config, &cancel).await);
        assert_eq!(
            Instant::now().duration_since(normal_started),
            Duration::from_millis(100)
        );
    }
}
