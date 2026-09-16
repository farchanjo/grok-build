//! Input, output, params and the `until` grammar for the `wait_for` tool.

use std::time::Duration;

use crate::types::params_validation::ParamValidationError;
use crate::types::resources::ResourceType;
use crate::util::duration::{format_duration, parse_duration, serde_opt_duration};

/// Canonical tool name advertised by `WaitForTool::id()`.
pub const WAIT_FOR_TOOL_NAME: &str = "wait_for";

/// Prefix the tool bakes into the watcher's `monitor_description`.
///
/// The pager keys the "Wait" row kind off it, so the producer here and the
/// consumer in `xai-grok-pager` share this one constant.
pub const WAIT_DESCRIPTION_PREFIX: &str = "wait: ";

/// Default blocking deadline for the inline phase (`[toolset.wait_for] timeout`).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
/// Default for the ceiling applied to any requested deadline (`[toolset.wait_for] max_timeout`).
/// Unlike [`DEFAULT_MAX_TIMEOUT`] it is configurable: a session may raise it up to the platform limits.
pub const DEFAULT_MAX_TIMEOUT: Duration = Duration::from_secs(600);
/// Default cap for a single attempt, inline or in the watcher.
pub const DEFAULT_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(30);
/// Default backoff base between watcher attempts.
pub const DEFAULT_RETRY_INITIAL: Duration = Duration::from_secs(1);
/// Default backoff ceiling between watcher attempts.
pub const DEFAULT_RETRY_MAX: Duration = Duration::from_secs(30);
/// Default backoff growth factor.
pub const DEFAULT_RETRY_MULTIPLIER: u32 = 2;
/// Default jitter, in permille of the computed delay (100 = ±10%).
pub const DEFAULT_RETRY_JITTER_PERMILLE: u32 = 100;

/// Everything that can go wrong before or during a wait.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WaitForError {
    #[error("`until` cannot be empty")]
    EmptyUntil,
    #[error("`until` {0:?} is missing a unit — write e.g. `5s`, or a command")]
    MissingUnit(String),
    #[error("`until` {0:?} has an unknown duration unit {1:?} (expected ms, s, m, h, or d)")]
    UnknownUnit(String, String),
    #[error("`until` duration {0} exceeds the deadline {1}")]
    DelayExceedsTimeout(String, String),
    #[error("`retry` {0} exceeds the deadline {1} — the watcher would never retry")]
    RetryExceedsTimeout(String, String),
    #[error("`retry` must be greater than zero")]
    ZeroRetry,
    #[error("wait_for params: {0}")]
    InvalidParams(String),
    #[error("missing resource: {0}")]
    MissingResource(String),
}

impl From<crate::util::duration::DurationParseError> for WaitForError {
    fn from(error: crate::util::duration::DurationParseError) -> Self {
        use crate::util::duration::DurationParseError as E;
        match error {
            E::Empty => Self::EmptyUntil,
            E::MissingUnit(raw) => Self::MissingUnit(raw),
            E::UnknownUnit(raw, unit) => Self::UnknownUnit(raw, unit),
            E::Malformed(raw) => Self::MissingUnit(raw),
            E::Overflow(raw) => Self::InvalidParams(format!("duration {raw:?} overflows")),
        }
    }
}

/// A parsed `until`: an optional initial delay and an optional condition command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Until {
    pub delay: Option<Duration>,
    pub command: Option<String>,
}

impl Until {
    /// True when there is nothing to poll — a pure delay.
    pub fn is_delay_only(&self) -> bool {
        self.command.is_none()
    }
}

/// Parse the `until` value.
///
/// Three shapes, resolved in order:
/// 1. all digits (`"60"`) → error, the unit is missing;
/// 2. a leading `<digits><unit>` → that delay, the remainder (after optional whitespace and
///    `&&`) is the condition command;
/// 3. anything else → the whole string is the condition command.
///
/// A lone token that looks like a duration but carries an unknown unit (`"5x"`) is an error so
/// typos stay loud. The same token followed by more words is a command, which is what makes
/// `7z t archive.7z` and `2to3 script.py` work.
pub fn parse_until(raw: &str) -> Result<Until, WaitForError> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(WaitForError::EmptyUntil);
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        return Err(WaitForError::MissingUnit(s.to_owned()));
    }

    let digits_len = s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits_len > 0 {
        // A leading run of `<digits><unit>` segments with no spaces between them is one compound
        // duration (`1m30s`, `1h5m10s`). The run stops at the first unrecognised unit, which is
        // what keeps `7z t x` and `2to3 x` commands.
        let mut delay = Duration::ZERO;
        let mut pos = 0usize;
        let mut segments = 0usize;
        let mut bad_unit: Option<String> = None;
        loop {
            let tail = &s[pos..];
            let dlen = tail.len() - tail.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            if dlen == 0 {
                break;
            }
            let after_digits = &tail[dlen..];
            let ulen = after_digits.len()
                - after_digits
                    .trim_start_matches(|c: char| c.is_ascii_alphabetic())
                    .len();
            if ulen == 0 {
                break;
            }
            let unit = &after_digits[..ulen];
            if !matches!(unit, "ms" | "s" | "m" | "h" | "d") {
                if segments == 0 {
                    bad_unit = Some(unit.to_owned());
                }
                break;
            }
            let end = pos + dlen + ulen;
            delay += parse_duration(&s[pos..end])?;
            pos = end;
            segments += 1;
        }

        if segments > 0 {
            let mut tail = s[pos..].trim_start();
            if let Some(stripped) = tail.strip_prefix("&&") {
                tail = stripped.trim_start();
            }
            let command = (!tail.is_empty()).then(|| tail.to_owned());
            return Ok(Until {
                delay: Some(delay),
                command,
            });
        }
        // A lone token that looks like a duration but carries an unknown unit is a typo, not a
        // command: `5x` errors, while `7z t x` (more words follow) stays a command.
        if let Some(unit) = bad_unit
            && s[unit.len() + 1..].trim().is_empty()
        {
            return Err(WaitForError::UnknownUnit(s.to_owned(), unit));
        }
    }

    Ok(Until {
        delay: None,
        command: Some(s.to_owned()),
    })
}

/// Model-facing input.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct WaitForInput {
    /// A duration (`"5s"`), a shell command whose exit code is the condition, or both
    /// (`"10s && curl -sf localhost:3000"`). Exit code 0 satisfies the wait.
    #[schemars(
        description = "A duration (\"5s\"), a shell command whose exit code is the condition, or both (\"10s && curl -sf localhost:3000\"). Exit code 0 satisfies the wait; any other exit code retries until the deadline."
    )]
    pub until: String,

    /// Base interval between attempts (`"2s"`). When set, the interval stays fixed.
    #[serde(default, with = "serde_opt_duration")]
    #[schemars(
        description = "Base interval between attempts (\"2s\"). When set, the interval stays fixed instead of backing off.",
        with = "String"
    )]
    pub retry: Option<Duration>,

    /// Deadline for the whole wait (`"120s"`). Defaults to the session config, clamped.
    #[serde(default, with = "serde_opt_duration")]
    #[schemars(
        description = "Deadline for the whole wait (\"120s\"). Defaults to the session config, clamped to its ceiling.",
        with = "String"
    )]
    pub timeout: Option<Duration>,

    /// Keep watching after the inline attempt, waking the agent on satisfaction.
    #[serde(default)]
    #[schemars(
        description = "Keep watching after the inline attempt and wake the agent when the condition is met. Defaults to true."
    )]
    pub wake: Option<bool>,
}

impl WaitForInput {
    /// Cross-field validation that needs the resolved params.
    ///
    /// `retry` and `watch` are the effective values (input over params), because
    /// both checks describe what the watcher would actually do.
    pub fn validate(
        &self,
        until: &Until,
        timeout: Duration,
        retry: Duration,
        watch: bool,
    ) -> Result<(), WaitForError> {
        if self.retry == Some(Duration::ZERO) {
            return Err(WaitForError::ZeroRetry);
        }
        if let Some(delay) = until.delay
            && delay > timeout
        {
            return Err(WaitForError::DelayExceedsTimeout(
                format_duration(delay),
                format_duration(timeout),
            ));
        }
        // A retry interval wider than the whole deadline leaves the watcher with
        // one attempt and a sleep: `retry: "60s"` against the default 120s is
        // fine, against `timeout: "30s"` it is a mistake worth naming.
        if watch && !until.is_delay_only() && retry > timeout {
            return Err(WaitForError::RetryExceedsTimeout(
                format_duration(retry),
                format_duration(timeout),
            ));
        }
        Ok(())
    }
}

/// How the wait ended.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WaitOutcome {
    /// The condition held (or the delay elapsed) inside the inline phase.
    Satisfied,
    /// The inline attempt failed; a watcher keeps polling in the background.
    Watching,
    /// The inline attempt failed, the deadline is still open, and `wake` was
    /// off — so nothing keeps polling. Distinct from `TimedOut`, which means
    /// the deadline itself ran out.
    NotSatisfied,
    /// The deadline was already exhausted when the tool returned.
    TimedOut,
}

/// Model-facing output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct WaitForOutput {
    pub outcome: WaitOutcome,
    /// The `until` value as given.
    pub until: String,
    /// The parsed initial delay, when the caller wrote one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<String>,
    /// The parsed condition command, when the caller wrote one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Attempts made inside the inline phase.
    pub attempts: u32,
    /// Wall time spent inside the inline phase.
    pub elapsed: String,
    /// Exit code of the last inline attempt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_exit_code: Option<i32>,
    /// Output of the last inline attempt, truncated.
    pub last_output: String,
    /// Watcher task id, present whenever a watcher was spawned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

impl xai_tool_runtime::ToolOutput for WaitForOutput {}

/// Runtime-configurable parameters (`[toolset.wait_for]`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct WaitForParams {
    /// Backoff base between watcher attempts.
    #[serde(with = "crate::util::duration::serde_duration")]
    #[schemars(with = "String")]
    pub retry_initial: Duration,
    /// Backoff ceiling between watcher attempts.
    #[serde(with = "crate::util::duration::serde_duration")]
    #[schemars(with = "String")]
    pub retry_max: Duration,
    /// Backoff growth factor per attempt.
    pub retry_multiplier: u32,
    /// Jitter in permille of the computed delay (100 = ±10%); 0 disables.
    pub retry_jitter_permille: u32,
    /// Default deadline when the model omits `timeout`.
    #[serde(with = "crate::util::duration::serde_duration")]
    #[schemars(with = "String")]
    pub timeout: Duration,
    /// Ceiling applied to any requested deadline.
    #[serde(with = "crate::util::duration::serde_duration")]
    #[schemars(with = "String")]
    pub max_timeout: Duration,
    /// Cap for a single attempt, inline or in the watcher.
    #[serde(with = "crate::util::duration::serde_duration")]
    #[schemars(with = "String")]
    pub attempt_timeout: Duration,
    /// Whether a watcher is spawned by default after the inline phase.
    pub wake_on_timeout: bool,
}

impl Default for WaitForParams {
    fn default() -> Self {
        Self {
            retry_initial: DEFAULT_RETRY_INITIAL,
            retry_max: DEFAULT_RETRY_MAX,
            retry_multiplier: DEFAULT_RETRY_MULTIPLIER,
            retry_jitter_permille: DEFAULT_RETRY_JITTER_PERMILLE,
            timeout: DEFAULT_TIMEOUT,
            max_timeout: DEFAULT_MAX_TIMEOUT,
            attempt_timeout: DEFAULT_ATTEMPT_TIMEOUT,
            wake_on_timeout: true,
        }
    }
}

impl WaitForParams {
    /// Resolve the effective deadline for this call.
    pub fn resolve_timeout(&self, requested: Option<Duration>) -> Duration {
        requested.unwrap_or(self.timeout).min(self.max_timeout)
    }
}

impl ResourceType for WaitForParams {
    const ID: &'static str = "grok_build.WaitFor";

    fn validate_params_value(value: &Self) -> Result<(), ParamValidationError> {
        let invalid = |field: &'static str, expected: &'static str, bad: String| {
            ParamValidationError::new(
                format!("wait_for params: {field} {bad} is invalid"),
                "params_constraint",
            )
            .with_field_path(field)
            .with_expected(expected)
            .with_bad_value(serde_json::Value::String(bad))
        };
        if value.retry_initial.is_zero() {
            return Err(invalid(
                "retry_initial",
                "a positive duration",
                "0s".to_owned(),
            ));
        }
        if value.retry_max < value.retry_initial {
            return Err(invalid(
                "retry_max",
                ">= retry_initial",
                format_duration(value.retry_max),
            ));
        }
        if value.retry_multiplier < 1 {
            return Err(invalid(
                "retry_multiplier",
                ">= 1",
                value.retry_multiplier.to_string(),
            ));
        }
        if value.retry_jitter_permille > 1000 {
            return Err(invalid(
                "retry_jitter_permille",
                "<= 1000",
                value.retry_jitter_permille.to_string(),
            ));
        }
        if value.timeout.is_zero() {
            return Err(invalid("timeout", "a positive duration", "0s".to_owned()));
        }
        if value.max_timeout < value.timeout {
            return Err(invalid(
                "max_timeout",
                ">= timeout",
                format_duration(value.max_timeout),
            ));
        }
        if value.attempt_timeout.is_zero() {
            return Err(invalid(
                "attempt_timeout",
                "a positive duration",
                "0s".to_owned(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(raw: &str) -> Until {
        parse_until(raw).expect("should parse")
    }

    #[test]
    fn duration_only() {
        let until = parsed("5s");
        assert_eq!(until.delay, Some(Duration::from_secs(5)));
        assert!(until.is_delay_only());
        assert_eq!(parsed("500ms").delay, Some(Duration::from_millis(500)));
    }

    #[test]
    fn delay_then_command_with_and_without_operator() {
        for raw in [
            "10s && curl -sf localhost:3000",
            "10s curl -sf localhost:3000",
        ] {
            let until = parsed(raw);
            assert_eq!(until.delay, Some(Duration::from_secs(10)));
            assert_eq!(until.command.as_deref(), Some("curl -sf localhost:3000"));
        }
    }

    #[test]
    fn command_only_is_immediate() {
        let until = parsed("gh run view --json status");
        assert_eq!(until.delay, None);
        assert_eq!(until.command.as_deref(), Some("gh run view --json status"));
    }

    #[test]
    fn digit_prefixed_binaries_stay_commands() {
        assert_eq!(
            parsed("7z t archive.7z").command.as_deref(),
            Some("7z t archive.7z")
        );
        assert_eq!(
            parsed("2to3 script.py").command.as_deref(),
            Some("2to3 script.py")
        );
        assert_eq!(parsed("7z t archive.7z").delay, None);
    }

    #[test]
    fn missing_unit_is_an_error() {
        assert_eq!(
            parse_until("60"),
            Err(WaitForError::MissingUnit("60".into()))
        );
    }

    #[test]
    fn lone_token_with_unknown_unit_is_an_error() {
        assert_eq!(
            parse_until("5x"),
            Err(WaitForError::UnknownUnit("5x".into(), "x".into()))
        );
    }

    #[test]
    fn empty_is_an_error() {
        assert_eq!(parse_until("   "), Err(WaitForError::EmptyUntil));
    }

    #[test]
    fn zero_delay_is_valid() {
        let until = parsed("0s");
        assert_eq!(until.delay, Some(Duration::ZERO));
        assert!(until.is_delay_only());
    }

    #[test]
    fn compound_durations_sum_their_segments() {
        assert_eq!(parsed("1m30s").delay, Some(Duration::from_secs(90)));
        assert_eq!(parsed("1h5m10s").delay, Some(Duration::from_secs(3_910)));
        assert!(parsed("1m30s").is_delay_only());
        let with_command = parsed("1m30s && curl -sf x");
        assert_eq!(with_command.delay, Some(Duration::from_secs(90)));
        assert_eq!(with_command.command.as_deref(), Some("curl -sf x"));
    }

    #[test]
    fn fractional_units_stay_commands_and_uppercase_units_error() {
        // Documented behaviour: the parser is integer-only, so `3.5s` has no unit it recognises
        // and falls through to the command branch. `5S` does look like a lone duration token,
        // so the case mistake is reported instead of silently running a command named `5S`.
        assert_eq!(parsed("3.5s").command.as_deref(), Some("3.5s"));
        assert_eq!(parsed("3.5s").delay, None);
        assert_eq!(
            parse_until("5S"),
            Err(WaitForError::UnknownUnit("5S".into(), "S".into()))
        );
    }

    #[test]
    fn digit_prefix_with_an_unknown_unit_and_no_space_is_a_typo() {
        assert_eq!(
            parse_until("5x"),
            Err(WaitForError::UnknownUnit("5x".into(), "x".into()))
        );
    }

    #[test]
    fn delay_beyond_timeout_is_rejected() {
        let until = parsed("10s");
        let input = WaitForInput {
            until: "10s".into(),
            retry: None,
            timeout: None,
            wake: None,
        };
        assert!(
            input
                .validate(&until, Duration::from_secs(5), Duration::from_secs(1), true)
                .is_err_and(|e| matches!(e, WaitForError::DelayExceedsTimeout(_, _)))
        );
        assert!(
            input
                .validate(
                    &until,
                    Duration::from_secs(30),
                    Duration::from_secs(1),
                    true
                )
                .is_ok()
        );
    }

    #[test]
    fn retry_wider_than_the_deadline_is_rejected() {
        let until = parsed("curl -sf x");
        let input = WaitForInput {
            until: "curl -sf x".into(),
            retry: Some(Duration::from_secs(60)),
            timeout: None,
            wake: None,
        };
        assert!(
            input
                .validate(
                    &until,
                    Duration::from_secs(30),
                    Duration::from_secs(60),
                    true
                )
                .is_err_and(|e| matches!(e, WaitForError::RetryExceedsTimeout(_, _)))
        );
        // Inside the deadline, and outside a watcher (no wake, or a pure delay),
        // the same retry is fine.
        assert!(
            input
                .validate(
                    &until,
                    Duration::from_secs(120),
                    Duration::from_secs(60),
                    true
                )
                .is_ok()
        );
        assert!(
            input
                .validate(
                    &until,
                    Duration::from_secs(30),
                    Duration::from_secs(60),
                    false
                )
                .is_ok()
        );
        let delay_only = parsed("5s");
        assert!(
            input
                .validate(
                    &delay_only,
                    Duration::from_secs(30),
                    Duration::from_secs(60),
                    true
                )
                .is_ok()
        );
    }

    #[test]
    fn zero_retry_is_rejected() {
        let until = parsed("cmd");
        let input = WaitForInput {
            until: "cmd".into(),
            retry: Some(Duration::ZERO),
            timeout: None,
            wake: None,
        };
        assert_eq!(
            input.validate(
                &until,
                Duration::from_secs(30),
                Duration::from_secs(1),
                true
            ),
            Err(WaitForError::ZeroRetry)
        );
    }

    #[test]
    fn params_defaults_are_valid() {
        assert!(WaitForParams::validate_params_value(&WaitForParams::default()).is_ok());
    }

    #[test]
    fn params_validation_catches_inverted_bounds() {
        let bad = WaitForParams {
            retry_initial: Duration::from_secs(10),
            retry_max: Duration::from_secs(5),
            ..Default::default()
        };
        assert!(WaitForParams::validate_params_value(&bad).is_err());

        let bad = WaitForParams {
            retry_multiplier: 0,
            ..Default::default()
        };
        assert!(WaitForParams::validate_params_value(&bad).is_err());
    }

    #[test]
    fn params_timeout_is_clamped_by_max() {
        let params = WaitForParams {
            timeout: Duration::from_secs(60),
            max_timeout: Duration::from_secs(30),
            ..Default::default()
        };
        assert_eq!(
            params.resolve_timeout(Some(Duration::from_secs(120))),
            Duration::from_secs(30)
        );
        assert_eq!(params.resolve_timeout(None), Duration::from_secs(30));
    }

    #[test]
    fn params_json_round_trips_through_duration_strings() {
        let params = WaitForParams::default();
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["timeout"], serde_json::json!("2m"));
        assert_eq!(json["retry_initial"], serde_json::json!("1s"));
        let back: WaitForParams = serde_json::from_value(json).unwrap();
        assert_eq!(back.timeout, params.timeout);
    }

    #[test]
    fn params_reject_a_bare_number() {
        let err = serde_json::from_value::<WaitForParams>(serde_json::json!({
            "timeout": 120
        }));
        assert!(err.is_err(), "a unit-less number must be rejected");
    }
}
