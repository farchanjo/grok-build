//! The log compressor: structural line elision with a verified marker.
//!
//! Strategy: split into lines; always keep the signal lines (lines that differ
//! in kind from their neighbors — errors, warnings, summary lines); elide runs
//! of near-identical boilerplate; feed every drop through
//! [`crate::compressors::elision::keep_non_redundant`] so a distinct record can
//! never be deleted by the class rule; finally replace each elided run with a
//! marker that states only verified facts ([`super::marker`]).
//!
//! Runs shorter than [`MIN_RUN`] lines are never elided: a one-line marker plus
//! its recovery handle costs about what the line cost and reads as a hole.

use super::Compressor;
use super::elision::{keep_non_redundant, normalized_shape};
use super::marker::{build_marker, parse_fields};
use crate::safety::Class;

/// The log content type registry key.
pub const CONTENT_TYPE: &str = "log";

/// A run below this length is not worth a marker.
const MIN_RUN: usize = 3;

/// Signal lines survive the class test even when their shape matches: an ERROR
/// among a thousand INFOs is the whole point of reading the log.
fn is_signal(line: &str) -> bool {
    static SIGNAL: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?i)\b(ERROR|FATAL|PANIC|CRITICAL|WARN(ING)?)\b")
            .expect("static signal regex")
    });
    SIGNAL.is_match(line)
}

/// Elide repeated lines out of `input`. Public so tests and other compressors
/// can reuse the run logic on their own unit shape.
#[must_use]
pub fn elide_lines(input: &str) -> Vec<u8> {
    let lines: Vec<&str> = input.split_inclusive('\n').collect();
    if lines.len() < 4 {
        return input.as_bytes().to_vec();
    }

    // Step 1: class-based representative pass. Units here are the lines with
    // their trailing newline normalized away for shape comparison.
    let bare: Vec<Vec<u8>> = lines
        .iter()
        .map(|l| l.trim_end_matches('\n').as_bytes().to_vec())
        .collect();
    let mut keep = vec![false; bare.len()];

    // Signal lines and the first/last lines are always kept.
    for (i, line) in lines.iter().enumerate() {
        if is_signal(line) || i == 0 || i + 1 == lines.len() {
            keep[i] = true;
        }
    }
    // Runs of MIN_RUN or more equal-shape lines: keep the first of the run and
    // let the rest elide (subject to the guard).
    let mut run_start = 0usize;
    for i in 1..=bare.len() {
        let same =
            i < bare.len() && normalized_shape(&bare[i]) == normalized_shape(&bare[run_start]);
        if !same {
            if i - run_start >= MIN_RUN && keep[run_start] {
                // Middle of the run: candidates for elision. Ends stay: the run
                // boundary is information (it started and it stopped).
                for slot in keep[run_start + 1..i - 1].iter_mut() {
                    *slot = false;
                }
            }
            run_start = i;
        }
    }
    // The guard upgrades any elision that would delete a distinct kind of line.
    keep_non_redundant(&bare, &mut keep);

    // Step 2: emit, replacing each elided run of >= MIN_RUN with a marker that
    // states what the dropped lines had in common.
    let mut out = String::with_capacity(input.len() / 2);
    let mut i = 0usize;
    while i < lines.len() {
        if keep[i] {
            out.push_str(lines[i]);
            i += 1;
            continue;
        }
        let run_start = i;
        while i < lines.len() && !keep[i] {
            i += 1;
        }
        let run = i - run_start;
        if run < MIN_RUN {
            for line in &lines[run_start..i] {
                out.push_str(line);
            }
            continue;
        }
        // Field views are owned per unit; the borrow checker is satisfied
        // without unsafe and the cost is bounded by the run size (which the
        // elision rule already capped).
        let mut observed: Vec<std::collections::BTreeMap<String, String>> = Vec::with_capacity(run);
        for line in &lines[run_start..i] {
            let trimmed = line.trim_end();
            let parsed = parse_fields(trimmed);
            observed.push(parsed.into_iter().collect());
        }
        let mut runs: std::collections::BTreeMap<String, Vec<&str>> =
            std::collections::BTreeMap::new();
        for obs in &observed {
            for (k, v) in obs {
                runs.entry(k.clone()).or_default().push(v.as_str());
            }
        }
        out.push_str(&build_marker(run, &runs));
        out.push('\n');
    }
    out.into_bytes()
}

/// The log compressor. S4: lossy, so the engine requires the original to be
/// recoverable before any elided bytes reach the model.
#[derive(Debug, Default)]
pub struct LogCompressor;

impl Compressor for LogCompressor {
    fn content_type(&self) -> &'static str {
        CONTENT_TYPE
    }

    fn safety_class(&self) -> Class {
        Class::S4
    }

    fn compress(&self, input: &[u8]) -> (Vec<u8>, bool) {
        let Ok(text) = std::str::from_utf8(input) else {
            return (Vec::new(), false);
        };
        let out = elide_lines(text);
        // Fail closed: no gain means nothing applied.
        if out.len() >= input.len() {
            return (Vec::new(), false);
        }
        (out, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repeated_log(n: usize) -> String {
        let mut s = String::from("2026-08-28 13:00:00 INFO boot start\n");
        for i in 0..n {
            s.push_str(&format!(
                "2026-08-28 13:00:{i:02} INFO worker status=ready attempt=1\n"
            ));
        }
        s.push_str("2026-08-28 13:01:00 ERROR disk full\n");
        s
    }

    #[test]
    fn collapses_a_long_boilerplate_run_into_one_survivor_plus_marker() {
        let input = repeated_log(20);
        let (out, ok) = LogCompressor.compress(input.as_bytes());
        assert!(ok);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("lines elided"), "{text}");
        assert!(
            text.contains("status=ready"),
            "marker must state the constant: {text}"
        );
        assert!(
            text.contains("ERROR disk full"),
            "signal line must survive: {text}"
        );
        assert!(out_is_shorter(&input, &text));
    }

    #[test]
    fn keeps_distinct_lines_whole_even_when_no_boilerplate_exists() {
        let input = "\
a distinct line about x
a distinct line about y
a distinct line about z
a distinct line about w
a distinct line about v
";
        let (out, ok) = LogCompressor.compress(input.as_bytes());
        assert!(!ok, "nothing distinct may be elided, so nothing applies");
        assert!(out.is_empty());
    }

    #[test]
    fn short_runs_are_not_worth_a_marker() {
        let input = "\
2026-08-28 13:00:00 INFO alpha status=1
2026-08-28 13:00:01 INFO alpha status=2
2026-08-28 13:00:02 INFO alpha status=3
2026-08-28 13:00:03 INFO tail
2026-08-28 13:00:04 INFO tail
2026-08-28 13:00:05 INFO tail
";
        let (out, ok) = LogCompressor.compress(input.as_bytes());
        if ok {
            let text = String::from_utf8(out).unwrap();
            // Any marker that appears must cover at least MIN_RUN lines.
            for seg in text.lines().filter(|l| l.contains("lines elided")) {
                let n: usize = seg
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0);
                assert!(n >= MIN_RUN || n == 0, "small marker leaked: {seg}");
            }
        }
    }

    #[test]
    fn utf8_invalid_input_fails_closed() {
        let (out, ok) = LogCompressor.compress(&[0xff, 0xfe, 0x00, 0x01]);
        assert!(!ok);
        assert!(out.is_empty());
    }

    #[test]
    fn compressor_is_registered_with_s4_class() {
        assert_eq!(LogCompressor.content_type(), "log");
        assert!(LogCompressor.safety_class().requires_recovery());
    }

    fn out_is_shorter(input: &str, out: &str) -> bool {
        out.len() < input.len()
    }
}
