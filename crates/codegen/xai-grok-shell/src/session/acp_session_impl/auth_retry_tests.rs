use std::time::Duration;

use xai_grok_inference_types::SentCredential;

use super::{AuthRetryDecision, AuthRetrySchedule, DualClock};

fn after_suspend(base: DualClock, wall_ahead: Duration) -> DualClock {
    DualClock {
        monotonic: base.monotonic,
        wall: base.wall + wall_ahead,
    }
}

#[test]
fn sent_and_unknown_exhaust_on_exact_one_two_four_schedule() {
    for credential in [SentCredential::Sent, SentCredential::Unknown] {
        let mut schedule = AuthRetrySchedule::new();
        let decisions: Vec<_> = (0..3)
            .map(|_| schedule.on_recovered_401(credential))
            .collect();
        assert_eq!(
            decisions,
            vec![
                AuthRetryDecision::Backoff {
                    attempt: 1,
                    delay: Duration::from_secs(1),
                },
                AuthRetryDecision::Backoff {
                    attempt: 2,
                    delay: Duration::from_secs(2),
                },
                AuthRetryDecision::Backoff {
                    attempt: 3,
                    delay: Duration::from_secs(4),
                },
            ],
            "{credential:?}",
        );
        assert_eq!(
            schedule.on_recovered_401(credential),
            AuthRetryDecision::Exhausted,
            "{credential:?}",
        );
        assert_eq!(
            schedule.incident_counts(),
            if credential == SentCredential::Sent {
                (4, 4)
            } else {
                (4, 0)
            },
            "Unknown charges but is not claimed authenticated",
        );
    }
}

#[test]
fn mixed_charged_provenance_shares_one_budget_without_overclaiming_auth() {
    let mut schedule = AuthRetrySchedule::new();
    for (credential, expected_attempt) in [
        (SentCredential::Unknown, 1),
        (SentCredential::Sent, 2),
        (SentCredential::Unknown, 3),
    ] {
        assert!(matches!(
            schedule.on_recovered_401(credential),
            AuthRetryDecision::Backoff { attempt, .. } if attempt == expected_attempt
        ));
    }
    assert_eq!(
        schedule.on_recovered_401(SentCredential::Sent),
        AuthRetryDecision::Exhausted,
    );
    assert_eq!(schedule.incident_counts(), (4, 2));
}

#[test]
fn missing_is_uncharged_but_runaway_guard_is_strictly_bounded() {
    let mut schedule = AuthRetrySchedule::new();
    for resubmit in 1..=AuthRetrySchedule::MAX_UNCHARGED_RESUBMITS {
        assert_eq!(
            schedule.on_recovered_401(SentCredential::Missing),
            AuthRetryDecision::UnchargedResubmit { resubmit },
        );
    }
    assert_eq!(
        schedule.on_recovered_401(SentCredential::Missing),
        AuthRetryDecision::RunawayGuard {
            rejections: AuthRetrySchedule::MAX_UNCHARGED_RESUBMITS + 1,
        },
    );
    assert_eq!(schedule.incident_counts(), (0, 0));
    assert_eq!(
        schedule.on_recovered_401(SentCredential::Sent),
        AuthRetryDecision::Backoff {
            attempt: 1,
            delay: Duration::from_secs(1),
        },
        "uncharged retries must not consume the ordinary budget",
    );
}

#[test]
fn actual_success_resets_every_counter_and_rearms_all_guards() {
    let mut schedule = AuthRetrySchedule::new();
    schedule.on_recovered_401(SentCredential::Sent);
    for _ in 0..AuthRetrySchedule::MAX_UNCHARGED_RESUBMITS {
        schedule.on_recovered_401(SentCredential::Missing);
    }
    let start = DualClock::now();
    let woke = after_suspend(start, Duration::from_secs(16 * 60));
    schedule.incident_started = Some(start);
    assert!(schedule.reset_if_incident_spans_suspend_at(woke));

    schedule.reset_on_success();
    assert_eq!(schedule.incident_counts(), (0, 0));
    assert_eq!(schedule.uncharged_rejections(), 0);
    assert_eq!(
        schedule.on_recovered_401(SentCredential::Missing),
        AuthRetryDecision::UnchargedResubmit { resubmit: 1 },
    );
    assert_eq!(
        schedule.on_recovered_401(SentCredential::Unknown),
        AuthRetryDecision::Backoff {
            attempt: 1,
            delay: Duration::from_secs(1),
        },
    );
}

#[test]
fn suspend_reset_preserves_uncharged_count_and_restarts_charged_incident() {
    let mut schedule = AuthRetrySchedule::new();
    let start = DualClock::now();
    schedule.on_recovered_401_at(SentCredential::Missing, start);
    schedule.on_recovered_401_at(SentCredential::Missing, start);
    schedule.on_recovered_401_at(SentCredential::Sent, start);

    let woke = after_suspend(start, Duration::from_secs(16 * 60));
    assert!(schedule.reset_if_incident_spans_suspend_at(woke));
    assert_eq!(
        schedule.on_recovered_401_at(SentCredential::Missing, woke),
        AuthRetryDecision::UnchargedResubmit { resubmit: 3 },
    );
    assert_eq!(
        schedule.on_recovered_401_at(SentCredential::Sent, woke),
        AuthRetryDecision::Backoff {
            attempt: 1,
            delay: Duration::from_secs(1),
        },
    );
}

#[test]
fn suspend_resets_are_capped_without_success_and_success_rearms_them() {
    let mut schedule = AuthRetrySchedule::new();
    let mut now = DualClock::now();
    for _ in 0..AuthRetrySchedule::MAX_SUSPEND_RESETS {
        schedule.on_recovered_401_at(SentCredential::Sent, now);
        now = after_suspend(now, Duration::from_secs(16 * 60));
        assert!(schedule.reset_if_incident_spans_suspend_at(now));
    }
    schedule.on_recovered_401_at(SentCredential::Sent, now);
    now = after_suspend(now, Duration::from_secs(16 * 60));
    assert!(!schedule.reset_if_incident_spans_suspend_at(now));

    schedule.reset_on_success();
    schedule.on_recovered_401_at(SentCredential::Sent, now);
    now = after_suspend(now, Duration::from_secs(16 * 60));
    assert!(schedule.reset_if_incident_spans_suspend_at(now));
}

#[test]
fn suspend_detection_requires_open_incident_and_real_positive_drift() {
    let mut schedule = AuthRetrySchedule::new();
    let start = DualClock::now();
    assert!(
        !schedule
            .reset_if_incident_spans_suspend_at(after_suspend(start, Duration::from_secs(3600),))
    );

    schedule.on_recovered_401_at(SentCredential::Sent, start);
    assert!(
        !schedule.reset_if_incident_spans_suspend_at(after_suspend(start, Duration::from_secs(5),))
    );
    let wall_back = DualClock {
        monotonic: start.monotonic + Duration::from_secs(5),
        wall: start.wall - Duration::from_secs(60),
    };
    assert!(!schedule.reset_if_incident_spans_suspend_at(wall_back));
    assert_eq!(
        schedule.on_recovered_401_at(SentCredential::Sent, start),
        AuthRetryDecision::Backoff {
            attempt: 2,
            delay: Duration::from_secs(2),
        },
        "failed suspend checks must not mutate the charged schedule",
    );
}
