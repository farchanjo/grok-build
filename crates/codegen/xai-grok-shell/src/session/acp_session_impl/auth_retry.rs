//! Per-turn accounting for 401s after a successful credential recovery.

use std::time::{Duration, Instant, SystemTime};

use tokio_retry::strategy::ExponentialBackoff;
use xai_grok_inference_types::SentCredential;

use super::RecoveredStore;
use crate::auth::AuthManager;

/// Pace an uncharged resubmit. Only a session-token recovery can usefully wait
/// for `AuthManager`; provider-owned credentials are floor-paced instead.
pub(crate) async fn pace_uncharged_resubmit(
    store: RecoveredStore,
    auth_manager: Option<&std::sync::Arc<AuthManager>>,
) {
    match (store, auth_manager) {
        (RecoveredStore::SessionToken, Some(am)) if am.current_wire_valid().is_none() => {
            am.wait_for_token_refresh(AuthRetrySchedule::UNCHARGED_REFRESH_WAIT)
                .await;
        }
        _ => tokio::time::sleep(AuthRetrySchedule::UNCHARGED_RESUBMIT_FLOOR).await,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthRetryDecision {
    UnchargedResubmit { resubmit: u32 },
    Backoff { attempt: u32, delay: Duration },
    Exhausted,
    RunawayGuard { rejections: u32 },
}

/// One instant captured on wall and monotonic clocks. Wall/monotonic drift is
/// used only for suspend detection; a backward wall adjustment cannot invent a
/// suspend because elapsed wall time clamps to zero.
#[derive(Clone, Copy)]
struct DualClock {
    monotonic: Instant,
    wall: SystemTime,
}

impl DualClock {
    fn now() -> Self {
        Self {
            monotonic: Instant::now(),
            wall: SystemTime::now(),
        }
    }

    fn elapsed_between(self, now: Self) -> (Duration, Duration) {
        (
            now.monotonic.saturating_duration_since(self.monotonic),
            now.wall.duration_since(self.wall).unwrap_or(Duration::ZERO),
        )
    }
}

/// Escalating retry budget scoped to one failure incident. Sent and unknown
/// credential provenance charge 1s/2s/4s slots. Missing provenance is
/// uncharged but paced and guarded independently.
pub(crate) struct AuthRetrySchedule {
    delays: std::iter::Take<ExponentialBackoff>,
    attempt: u32,
    incident_rejections: u32,
    incident_authenticated: u32,
    incident_started: Option<DualClock>,
    uncharged_resubmits: u32,
    suspend_resets: u32,
}

impl AuthRetrySchedule {
    pub(crate) const MAX_RETRIES: u32 = 3;
    pub(crate) const MAX_UNCHARGED_RESUBMITS: u32 = 50;
    pub(crate) const MAX_SUSPEND_RESETS: u32 = 8;
    const UNCHARGED_REFRESH_WAIT: Duration = Duration::from_secs(15);
    const UNCHARGED_RESUBMIT_FLOOR: Duration = Duration::from_secs(1);
    const SUSPEND_DRIFT_MIN: Duration = Duration::from_secs(30);

    pub(crate) fn new() -> Self {
        Self {
            // `from_millis` raises the base to the attempt number. This exact
            // spelling yields 1s, 2s, 4s; using 1000 yields pathological waits.
            delays: ExponentialBackoff::from_millis(2)
                .factor(500)
                .max_delay(Duration::from_secs(10))
                .take(Self::MAX_RETRIES as usize),
            attempt: 0,
            incident_rejections: 0,
            incident_authenticated: 0,
            incident_started: None,
            uncharged_resubmits: 0,
            suspend_resets: 0,
        }
    }

    pub(crate) fn on_recovered_401(&mut self, credential: SentCredential) -> AuthRetryDecision {
        self.on_recovered_401_at(credential, DualClock::now())
    }

    fn on_recovered_401_at(
        &mut self,
        credential: SentCredential,
        now: DualClock,
    ) -> AuthRetryDecision {
        if credential.is_missing() {
            self.uncharged_resubmits = self.uncharged_resubmits.saturating_add(1);
            if self.uncharged_resubmits > Self::MAX_UNCHARGED_RESUBMITS {
                return AuthRetryDecision::RunawayGuard {
                    rejections: self.uncharged_resubmits,
                };
            }
            return AuthRetryDecision::UnchargedResubmit {
                resubmit: self.uncharged_resubmits,
            };
        }

        self.incident_started.get_or_insert(now);
        self.incident_rejections = self.incident_rejections.saturating_add(1);
        if credential == SentCredential::Sent {
            self.incident_authenticated = self.incident_authenticated.saturating_add(1);
        }
        match self.delays.next() {
            Some(delay) => {
                self.attempt = self.attempt.saturating_add(1);
                AuthRetryDecision::Backoff {
                    attempt: self.attempt,
                    delay,
                }
            }
            None => AuthRetryDecision::Exhausted,
        }
    }

    /// Restart the charged incident when it spans a real suspend, capped per
    /// success-free stretch. The uncharged runaway count intentionally
    /// survives; otherwise repeated suspend cycles could retry forever.
    pub(crate) fn reset_if_incident_spans_suspend(&mut self) -> bool {
        self.reset_if_incident_spans_suspend_at(DualClock::now())
    }

    fn reset_if_incident_spans_suspend_at(&mut self, now: DualClock) -> bool {
        let Some(started) = self.incident_started else {
            return false;
        };
        if self.suspend_resets >= Self::MAX_SUSPEND_RESETS {
            return false;
        }
        let (awake, wall) = started.elapsed_between(now);
        if wall.saturating_sub(awake) < Self::SUSPEND_DRIFT_MIN {
            return false;
        }

        let uncharged = self.uncharged_resubmits;
        let resets = self.suspend_resets;
        *self = Self::new();
        self.uncharged_resubmits = uncharged;
        self.suspend_resets = resets + 1;
        true
    }

    /// Only an actual successful inference response resets every counter and
    /// re-arms suspend recovery.
    pub(crate) fn reset_on_success(&mut self) {
        *self = Self::new();
    }

    pub(crate) fn incident_counts(&self) -> (u32, u32) {
        (self.incident_rejections, self.incident_authenticated)
    }

    pub(crate) fn uncharged_rejections(&self) -> u32 {
        self.uncharged_resubmits
    }
}

#[cfg(test)]
#[path = "auth_retry_tests.rs"]
mod tests;
