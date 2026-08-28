//! The elision guard: `keep_non_redundant`.
//!
//! # Why this exists
//!
//! An elider keeps the first and last few units of a payload plus whatever
//! carries signal, and drops the rest. On a payload dominated by repeated
//! boilerplate, the dropped units are represented by the kept ones, the model
//! can still answer, and the reduction is real. On a payload where every unit
//! is distinct — a bibliography, a source file, a table of measurements — the
//! same transform silently deletes most of the answer. The model then has to
//! recover the source repeatedly, which erases any token saving and can end the
//! turn with a worse answer than simply not compressing.
//!
//! # The rule
//!
//! A unit about to be dropped is kept when no unit already surviving resembles
//! it; its later copies still elide against it. This separates *eliding
//! repetition* from *truncating a document*, and it is a correctness rule, not
//! a tuning knob. Keeping one representative of each distinct class — rather
//! than refusing to compress at all — is what makes the rule useful: a payload
//! that is half boilerplate and half distinct records still gets its
//! boilerplate collapsed.
//!
//! # Resemblance
//!
//! Two units resemble each other when their normalized shapes match.
//! Normalization masks digit runs, because a log line that differs only in
//! counters and timestamps is structurally the same line. Masking applies only
//! to units carrying at least one word: in a wordless unit like
//! `6 ['1973.', 251]` the numbers *are* the content, so digits stay intact
//! there.
//!
//! Determinism is load-bearing: the kept set must be a pure function of the
//! unit bytes in document order, or a compressed block would re-serialize
//! differently across turns and bust the provider prefix cache.

/// Masks every run of digits in `text` with a single marker character, but only
/// when the unit carries at least one alphabetic word. Returns the normalized
/// shape used for resemblance matching.
#[must_use]
pub fn normalized_shape(unit: &[u8]) -> Vec<u8> {
    let has_word = unit.iter().any(u8::is_ascii_alphabetic);
    let mut out = Vec::with_capacity(unit.len());
    let mut in_digits = false;
    for &b in unit {
        if b.is_ascii_digit() {
            if !has_word {
                out.push(b);
            } else if !in_digits {
                out.push(b'#');
                in_digits = true;
            }
        } else {
            in_digits = false;
            out.push(b);
        }
    }
    out
}

/// Decide which units survive, in document order.
///
/// `keep` is an initial candidate decision (for example "keep the first three
/// and last three, drop the middle"). The guard upgrades it: every unit whose
/// normalized shape is not yet represented among the kept units becomes kept.
/// The result guarantees that every distinct kind of content in the payload
/// survives at least once.
pub fn keep_non_redundant(units: &[Vec<u8>], keep: &mut [bool]) {
    if units.len() != keep.len() {
        // A caller with mismatched slices has a bug; refuse to drop anything
        // rather than make a decision over data we cannot index.
        for slot in keep.iter_mut() {
            *slot = true;
        }
        return;
    }
    let mut seen: Vec<Vec<u8>> = Vec::new();
    for (unit, slot) in units.iter().zip(keep.iter_mut()) {
        if *slot {
            let shape = normalized_shape(unit);
            if !seen.contains(&shape) {
                seen.push(shape);
            }
            continue;
        }
        let shape = normalized_shape(unit);
        if !seen.contains(&shape) {
            // Nothing surviving represents this unit: dropping it would delete
            // a distinct kind of content.
            *slot = true;
            seen.push(shape);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(lines: &[&str]) -> Vec<Vec<u8>> {
        lines.iter().map(|l| l.as_bytes().to_vec()).collect()
    }

    #[test]
    fn repeated_boilerplate_still_elides_against_its_representative() {
        let u = units(&[
            "2026-08-28 13:00:01 INFO worker started",
            "2026-08-28 13:00:02 INFO worker started",
            "2026-08-28 13:00:03 INFO worker started",
            "2026-08-28 13:00:04 INFO worker started",
        ]);
        let mut keep = vec![true, false, false, false];
        keep_non_redundant(&u, &mut keep);
        assert_eq!(keep, vec![true, false, false, false]);
    }

    #[test]
    fn digit_only_variation_is_the_same_shape_and_elides() {
        let u = units(&[
            "GET /api/items 200 in 12ms",
            "GET /api/items 200 in 37ms",
            "GET /api/items 404 in 8ms",
        ]);
        let mut keep = vec![true, false, false];
        keep_non_redundant(&u, &mut keep);
        // 200/37/12 mask to the same shape; 404 sits in a word-bearing unit so
        // it masks too — all three are structurally one line.
        assert_eq!(keep, vec![true, false, false]);
    }

    #[test]
    fn refuses_to_elide_a_document_of_distinct_records() {
        let u = units(&[
            "@article{Jumper2021, title = {AlphaFold}}",
            "@inproceedings{patel2023, title = {Blockchain}}",
            "@article{Watson1953, title = {Nucleic Acid}}",
            "@article{Vaswani2017, title = {Attention}}",
        ]);
        // A naive elider would keep only the first: the guard must keep all.
        let mut keep = vec![true, false, false, false];
        keep_non_redundant(&u, &mut keep);
        assert_eq!(keep, vec![true, true, true, true]);
    }

    #[test]
    fn wordless_units_keep_their_digits() {
        // The numbers ARE the content in a data row; masking them would make
        // different rows look alike and lose the measurements.
        let u = units(&["6 ['1973.', 251]", "7 ['1984.', 310]"]);
        let mut keep = vec![true, false];
        keep_non_redundant(&u, &mut keep);
        assert_eq!(keep, vec![true, true]);
    }

    #[test]
    fn half_boilerplate_half_distinct_collapses_only_the_boilerplate() {
        let mut lines: Vec<String> = Vec::new();
        lines.push("incident opened".to_string());
        for i in 0..8 {
            lines.push(format!("2026-08-28 13:00:0{i} INFO heartbeat ok"));
        }
        lines.push("root cause: expired TLS cert on edge-3".to_string());
        let owned: Vec<&str> = lines.iter().map(String::as_str).collect();
        let u = units(&owned);
        let mut keep = vec![true; u.len()];
        // Naive middle-drop: everything between the ends is dropped.
        for slot in keep.iter_mut().take(u.len() - 1).skip(1) {
            *slot = false;
        }
        keep_non_redundant(&u, &mut keep);
        let dropped = keep.iter().filter(|k| !**k).count();
        // Eight heartbeats collapse to one representative; the distinct lines
        // survive, so exactly the seven redundant copies may drop.
        assert_eq!(dropped, 7, "keep flags: {keep:?}");
    }

    #[test]
    fn mismatched_slice_lengths_keeps_everything() {
        let u = units(&["a", "b"]);
        let mut keep = vec![false];
        keep_non_redundant(&u, &mut keep);
        assert_eq!(keep, vec![true]);
    }

    #[test]
    fn decision_is_a_pure_function_of_order_and_bytes() {
        let u = units(&["x 1", "x 2", "y"]);
        let mut a = vec![true, false, false];
        let mut b = vec![true, false, false];
        keep_non_redundant(&u, &mut a);
        keep_non_redundant(&u, &mut b);
        assert_eq!(a, b);
    }
}
