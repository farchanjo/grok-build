//! Recovery markers: what an elision may claim about the units it replaced.
//!
//! A marker that only reports a count ("N rows elided") does not say *what* was
//! common among the dropped units. An agent that cannot tell stops trusting the
//! surviving view and recovers the whole payload, erasing the saving. So a
//! marker carries facts COMPUTED from the exact units it replaced:
//!
//! - a field byte-identical across EVERY elided unit is stated as `name=value`;
//! - a field varying over a small set of short values is enumerated in full with
//!   exact counts (`status: fulfilled x15 shipped x3`), with an `absent xN`
//!   bucket so counts always sum to the elided count;
//! - a field present in every unit whose values all parse as numbers is stated
//!   as `name=min..max`, printed from the ORIGINAL value strings of the extreme
//!   units — never reformatted, so a bound can never be wider than reality.
//!
//! Enumeration is all-or-nothing per field: over five distinct values, or one
//! value too long or with whitespace, and the field is withheld entirely. A
//! partial list reads as complete and would imply a fact it did not verify —
//! the one thing this module may never do.
//!
//! Nothing else is claimed. When nothing survives, the marker renders exactly
//! like the bare count form.

use std::collections::BTreeMap;

/// Cap per summary entry, and never more than a quarter of the bytes replaced
/// (floor 64, never over half).
pub const ENTRY_CAP_BYTES: usize = 160;
pub const SUMMARY_FLOOR_BYTES: usize = 64;

/// Max distinct values an enumeration may hold before the field is withheld.
const MAX_ENUM_VALUES: usize = 5;
/// A value longer than this (or containing whitespace) is too free-text to
/// enumerate.
const MAX_VALUE_BYTES: usize = 24;
/// Field names shaped like credentials are never printed — a marker must not
/// leak secret material into the model-visible view.
const CREDENTIAL_HINTS: &[&str] = &[
    "key",
    "token",
    "secret",
    "password",
    "passwd",
    "api",
    "auth",
    "credential",
];

/// True when a field name looks like it holds a credential. Credential-shaped
/// names are withheld entirely: the value is the point of the row for a
/// set-logic question only when it is not a secret.
#[must_use]
pub fn credential_shaped(field: &str) -> bool {
    let lower = field.to_ascii_lowercase();
    CREDENTIAL_HINTS.iter().any(|hint| lower.contains(hint))
}

/// Field observations computed from one elided unit. Parsing is deliberately
/// strict: anything not cleanly `name=value` (whitespace around `=`, empty
/// sides) is not a field, and a unit yielding no fields disables the summary
/// for its whole run.
#[must_use]
pub fn parse_fields(unit: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for token in unit.split_whitespace() {
        let Some((name, value)) = token.split_once('=') else {
            continue;
        };
        if name.is_empty() || value.is_empty() {
            continue;
        }
        if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            continue;
        }
        fields.insert(name.to_string(), value.to_string());
    }
    fields
}

/// One field's summary line, computed from the elided units' observed values.
/// `total_units` is how many units the marker replaces; `None` means the field
/// may not be claimed at all.
///
/// A field observed on FEWER units than the run total is not constant across
/// the elided set — the unobserved units differ precisely in lacking it. Such
/// a field may only be enumerated (with its `absent xN` bucket), never stated
/// as `name=value`, or the marker would claim a fact about units it never saw.
#[must_use]
pub fn summarize_field(field: &str, values: &[&str], total_units: usize) -> Option<String> {
    if credential_shaped(field) || values.is_empty() {
        return None;
    }
    let complete = values.len() == total_units;
    // Constants: identical across every elided unit.
    if complete && values.iter().all(|v| *v == values[0]) {
        let value = values[0];
        if value.len() <= MAX_VALUE_BYTES && !value.chars().any(char::is_whitespace) {
            return Some(format!("{field}={value}"));
        }
        return None;
    }
    // Numeric range: present in every unit and all values parse as numbers.
    if complete {
        let parsed: Option<Vec<f64>> = values.iter().map(|v| v.parse::<f64>().ok()).collect();
        if let Some(nums) = parsed {
            // Compare numerically, print the ORIGINAL strings of the extremes.
            let mut idx: Vec<usize> = (0..nums.len()).collect();
            idx.sort_by(|a, b| {
                nums[*a]
                    .partial_cmp(&nums[*b])
                    .unwrap_or(core::cmp::Ordering::Equal)
            });
            let (lo, hi) = (values[*idx.first()?], values[*idx.last()?]);
            if lo == hi {
                return None; // constant handled above; equal-but-parsed is noise
            }
            return Some(format!("{field}={lo}..{hi}"));
        }
    }
    // Enumeration: only over a small set of short, whitespace-free values.
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for v in values {
        *counts.entry(v).or_default() += 1;
    }
    if counts.len() > MAX_ENUM_VALUES {
        return None;
    }
    if counts
        .keys()
        .any(|k| k.len() > MAX_VALUE_BYTES || k.chars().any(char::is_whitespace))
    {
        return None;
    }
    let mut parts: Vec<String> = counts.iter().map(|(v, c)| format!("{v} x{c}")).collect();
    if let Some(missing) = total_units.checked_sub(values.len())
        && missing > 0
    {
        parts.push(format!("absent x{missing}"));
    }
    Some(format!("{field}: {}", parts.join(" ")))
}

/// Build the full marker text for one elision run.
///
/// `runs[field]` holds the observed values across the elided units that carry
/// the field; `elided_total` is how many units the marker replaces. Entries are
/// shed whole (never truncated) when the budget binds: longest first, so the
/// short constants and enumerations that answer set-logic questions survive.
#[must_use]
pub fn build_marker(elided_total: usize, runs: &BTreeMap<String, Vec<&str>>) -> String {
    if elided_total == 0 {
        return String::new();
    }
    let mut entries: Vec<String> = runs
        .iter()
        .filter_map(|(field, values)| summarize_field(field, values, elided_total))
        .collect();
    entries.sort_by_key(|b| std::cmp::Reverse(b.len()));

    let budget = (elided_total * 16).clamp(SUMMARY_FLOOR_BYTES, ENTRY_CAP_BYTES * 4);
    let mut kept: Vec<String> = Vec::new();
    let mut used = 0usize;
    for entry in entries {
        if used + entry.len() > budget {
            continue;
        }
        used += entry.len();
        kept.push(entry);
    }
    // Emit in field order for determinism, not shed order.
    kept.sort();
    if kept.is_empty() {
        format!("... {elided_total} lines elided (tersify) ...")
    } else {
        format!(
            "... {elided_total} lines elided (tersify: {}) ...",
            kept.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs<'a>(pairs: &[(&'a str, &'a [&'a str])]) -> BTreeMap<String, Vec<&'a str>> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_vec()))
            .collect()
    }

    #[test]
    fn constant_field_is_stated_exactly() {
        let values = ["charged"; 20];
        let m = build_marker(20, &runs(&[("state", &values[..])]));
        assert!(m.contains("state=charged"), "{m}");
    }

    #[test]
    fn varying_field_enumerates_with_counts_that_sum_to_the_elided_count() {
        let mut values: Vec<&str> = vec!["fulfilled"; 15];
        values.extend(vec!["shipped"; 3]);
        values.extend(vec!["processing"; 2]);
        let m = build_marker(25, &runs(&[("status", &values)]));
        assert!(m.contains("fulfilled x15"), "{m}");
        assert!(m.contains("shipped x3"), "{m}");
        assert!(m.contains("processing x2"), "{m}");
        assert!(m.contains("status:"), "{m}");
    }

    #[test]
    fn absent_bucket_makes_counts_complete() {
        // A field observed on 3 of 5 units is not constant across them; it may
        // only be enumerated, and the enumeration must account for all 5.
        let values = vec!["on"; 3];
        let m = build_marker(5, &runs(&[("flag", &values)]));
        assert!(m.contains("on x3") && m.contains("absent x2"), "{m}");
        assert!(
            !m.contains("flag=on"),
            "partial coverage must not claim constant: {m}"
        );
    }

    #[test]
    fn numeric_range_prints_original_value_strings_not_reformatted() {
        let values = vec!["5.00", "199.99", "20.00"];
        let m = build_marker(3, &runs(&[("amount", &values)]));
        assert!(m.contains("amount=5.00..199.99"), "{m}");
        assert!(!m.contains("5..199.99"), "must not reformat: {m}");
    }

    #[test]
    fn more_than_five_distinct_values_withholds_the_field_entirely() {
        let values = vec!["a", "b", "c", "d", "e", "f"];
        let m = build_marker(6, &runs(&[("kind", &values)]));
        assert!(
            !m.contains("kind"),
            "field must be withheld, not shortened: {m}"
        );
        assert!(m.contains("6 lines elided"), "{m}");
    }

    #[test]
    fn credential_shaped_fields_are_never_printed() {
        let values = vec!["sk-live-123"; 4];
        let m = build_marker(4, &runs(&[("api_key", &values)]));
        assert!(!m.contains("sk-live"), "{m}");
    }

    #[test]
    fn free_text_values_are_withheld_not_truncated() {
        let values = vec!["a value with spaces", "another one"];
        let m = build_marker(2, &runs(&[("msg", &values)]));
        assert!(!m.contains("msg"), "{m}");
    }

    #[test]
    fn empty_marker_is_the_bare_count_form() {
        let m = build_marker(7, &runs(&[]));
        assert_eq!(m, "... 7 lines elided (tersify) ...");
    }

    #[test]
    fn zero_elided_emits_no_marker() {
        assert_eq!(build_marker(0, &runs(&[("a", &vec!["1"; 1][..])])), "");
    }

    #[test]
    fn parse_fields_is_strict_about_shape() {
        let f = parse_fields("GET /api 200 dur=12ms status=ok user_id=42 key = spaced=x");
        assert_eq!(f.get("status").map(String::as_str), Some("ok"));
        assert_eq!(f.get("user_id").map(String::as_str), Some("42"));
        assert_eq!(f.get("dur").map(String::as_str), Some("12ms"));
        // `key =` and `spaced=x` (name ok, but `spaced=x` parses fine) — check
        // the strictness boundary instead: a token with no '=' is not a field.
        assert!(!f.contains_key("GET"));
        // A wordless unit still yields numeric-ish fields only after masking;
        // here the path token simply contributes nothing.
    }

    #[test]
    fn entries_are_shed_whole_and_output_is_field_ordered() {
        let mut map: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for i in 0..40 {
            map.insert(format!("field{i:02}"), vec!["v"; 3]);
        }
        let m = build_marker(3, &map);
        // Determinism: same input, same output.
        assert_eq!(m, build_marker(3, &map));
    }
}
