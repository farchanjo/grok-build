//! Shared `Duration` grammar for time-valued tool inputs, tool params, and config.
//!
//! One grammar serves everything the `wait_for` feature touches: the `until` prefix, the
//! `timeout` / `retry` tool fields, and the `[toolset.wait_for]` config keys. Keeping a single
//! parser means the model-facing schema and the TOML surface cannot drift.
//!
//! Grammar: `<digits><unit>` with `unit` one of `ms`, `s`, `m`, `h`, `d`. The unit is
//! mandatory — a bare `"60"` is an error rather than a silent 60-second guess.
//!
//! Deliberately not `humantime`: that crate also accepts `"1m 30s"`, whose embedded space
//! collides with the `until` "duration then command" split.
//!
//! Deliberately not `scheduler::interval::parse_interval`: that one clamps to a 60-second
//! minimum and has no `ms`, so `until: "5s"` would wait a minute.

use std::time::Duration;

/// A time string that could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DurationParseError {
    #[error("duration cannot be empty")]
    Empty,
    #[error("duration {0:?} is missing a unit (expected ms, s, m, h, or d)")]
    MissingUnit(String),
    #[error("duration {0:?} is not a number followed by a unit (expected e.g. 500ms, 30s, 5m)")]
    Malformed(String),
    #[error("duration {0:?} has an unknown unit {1:?} (expected ms, s, m, h, or d)")]
    UnknownUnit(String, String),
    #[error("duration {0:?} overflows a Duration")]
    Overflow(String),
}

/// Parse `"<digits><unit>"` into a [`Duration`]. No minimum, no rounding.
pub fn parse_duration(input: &str) -> Result<Duration, DurationParseError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(DurationParseError::Empty);
    }

    let digits_len = s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let (digits, unit) = s.split_at(digits_len);
    if digits.is_empty() {
        return Err(DurationParseError::Malformed(s.to_owned()));
    }
    if unit.is_empty() {
        return Err(DurationParseError::MissingUnit(s.to_owned()));
    }

    let value: u32 = digits
        .parse()
        .map_err(|_| DurationParseError::Overflow(s.to_owned()))?;
    if value == 0 {
        return Ok(Duration::ZERO);
    }

    let unit_duration = match unit {
        "ms" => Duration::from_millis(1),
        "s" => Duration::from_secs(1),
        "m" => Duration::from_secs(60),
        "h" => Duration::from_secs(3_600),
        "d" => Duration::from_secs(86_400),
        other => {
            return Err(DurationParseError::UnknownUnit(
                s.to_owned(),
                other.to_owned(),
            ));
        }
    };

    unit_duration
        .checked_mul(value)
        .ok_or_else(|| DurationParseError::Overflow(s.to_owned()))
}

/// Canonical unit-bearing rendering, using the largest unit that divides evenly.
///
/// Sub-millisecond precision is truncated to milliseconds, so
/// `parse_duration(&format_duration(d)) == Ok(d)` holds for every `Duration` produced by
/// [`parse_duration`] and for whole-millisecond computed values.
pub fn format_duration(value: Duration) -> String {
    let millis = value.as_millis();
    if millis == 0 {
        return "0s".to_owned();
    }
    if millis % 1000 != 0 {
        return format!("{millis}ms");
    }
    let secs = value.as_secs();
    if secs % 86_400 == 0 {
        format!("{}d", secs / 86_400)
    } else if secs % 3_600 == 0 {
        format!("{}h", secs / 3_600)
    } else if secs % 60 == 0 {
        format!("{}m", secs / 60)
    } else {
        format!("{secs}s")
    }
}

/// `serde(with = …)` helpers for a required `Duration` field.
pub mod serde_duration {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    use super::{format_duration, parse_duration};

    pub fn serialize<S: Serializer>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format_duration(*value))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        let raw = String::deserialize(deserializer)?;
        parse_duration(&raw).map_err(serde::de::Error::custom)
    }
}

/// `serde(with = …)` helpers for an optional `Duration` field.
pub mod serde_opt_duration {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    use super::{format_duration, parse_duration};

    pub fn serialize<S: Serializer>(
        value: &Option<Duration>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(d) => serializer.serialize_some(&format_duration(*d)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Duration>, D::Error> {
        let raw = Option::<String>::deserialize(deserializer)?;
        match raw {
            Some(s) => parse_duration(&s)
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_unit() {
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(7_200));
        assert_eq!(parse_duration("1d").unwrap(), Duration::from_secs(86_400));
        assert_eq!(parse_duration("  5m  ").unwrap(), Duration::from_secs(300));
    }

    #[test]
    fn zero_is_valid_and_means_no_delay() {
        assert_eq!(parse_duration("0s").unwrap(), Duration::ZERO);
        assert_eq!(parse_duration("0ms").unwrap(), Duration::ZERO);
    }

    #[test]
    fn missing_unit_is_an_error() {
        assert_eq!(
            parse_duration("60"),
            Err(DurationParseError::MissingUnit("60".into()))
        );
    }

    #[test]
    fn unknown_unit_is_an_error() {
        assert!(matches!(
            parse_duration("5x"),
            Err(DurationParseError::UnknownUnit(_, _))
        ));
    }

    #[test]
    fn empty_and_malformed_are_errors() {
        assert_eq!(parse_duration(""), Err(DurationParseError::Empty));
        assert_eq!(parse_duration("   "), Err(DurationParseError::Empty));
        assert!(matches!(
            parse_duration("m"),
            Err(DurationParseError::Malformed(_))
        ));
        assert!(matches!(
            parse_duration("5s5"),
            Err(DurationParseError::UnknownUnit(_, _))
        ));
    }

    #[test]
    fn overflow_is_an_error_not_a_wrap() {
        assert!(matches!(
            parse_duration("1000000000000000000d"),
            Err(DurationParseError::Overflow(_))
        ));
    }

    #[test]
    fn format_prefers_the_largest_even_unit() {
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(300)), "5m");
        assert_eq!(format_duration(Duration::from_secs(7_200)), "2h");
        assert_eq!(format_duration(Duration::from_secs(86_400)), "1d");
        assert_eq!(format_duration(Duration::ZERO), "0s");
    }

    #[test]
    fn round_trips_ms_multiples() {
        for raw in ["500ms", "1s", "90s", "5m", "2h", "1d", "0s"] {
            let parsed = parse_duration(raw).unwrap();
            assert_eq!(parse_duration(&format_duration(parsed)).unwrap(), parsed);
        }
    }
}
