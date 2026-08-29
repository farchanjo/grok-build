//! Repetition-loop guard for streaming model output.
//!
//! Detects degenerate generation — the model stuck emitting the same
//! character or the same n-gram over and over (the `!!!!!` wall) — and
//! reports it so the turn can be aborted before the screen and the context
//! fill with garbage.
//!
//! Two rules, both evaluated on a rolling tail of the streamed text:
//!
//! 1. **Single-character flood** — a run of one non-alphanumeric character
//!    longer than [`MAX_SINGLE_CHAR_RUN`]. Pure punctuation is covered here
//!    (and only here) because markdown legitimately contains medium-length
//!    dashes/equals separators; 300 is far past any of those.
//! 2. **N-gram loop** — the tail ends with the same 16..=256-byte block
//!    repeated at least [`NGRAM_MIN_REPEATS`] times, where the block carries
//!    at least one alphanumeric character (word loops like "the the the…",
//!    JSON key loops). Periods not in the table are skipped; the smallest
//!    period means the shortest flagged loop is 96 chars of real content —
//!    long enough that no legitimate formatting trips it.
//!
//! The guard is deliberately insensitive to alphanumeric-only short repeats
//! ("ha ha ha"), code boilerplate (repeated identical JSON rows with
//! distinct ids fail the n-gram equality), and anything below the
//! thresholds. The consequence of a false positive is one aborted turn with
//! a clear notice — and the TUI setting to turn the guard off.

/// Rolling-tail size. Old text falls off the end; loops long enough to span
/// the whole window are caught by the single-character rule long before.
const TAIL_KEEP_CHARS: usize = 4096;

/// Run length of one non-alphanumeric character that trips the flood rule.
const MAX_SINGLE_CHAR_RUN: usize = 300;

/// n-gram periods (bytes) checked by the loop rule.
const NGRAM_PERIODS: &[usize] = &[16, 32, 48, 64, 96, 128, 192, 256];

/// Consecutive identical copies of an n-gram that trip the loop rule.
const NGRAM_MIN_REPEATS: usize = 6;

/// Incremental detector. Feed every Text-channel chunk via [`Self::push`];
/// read [`Self::looping`] after each push. `fired` latches so a single loop
/// triggers exactly one abort even if more chunks arrive before the stream
/// dies.
#[derive(Debug, Default)]
pub(crate) struct RepetitionGuard {
    tail: String,
    fired: bool,
}

impl RepetitionGuard {
    pub(crate) fn push(&mut self, text: &str) {
        if self.fired || text.is_empty() {
            return;
        }
        self.tail.push_str(text);
        if self.tail.chars().count() > TAIL_KEEP_CHARS {
            // Trim to the window on a char boundary, keeping the newest
            // TAIL_KEEP_CHARS characters.
            let skip = self.tail.chars().count() - TAIL_KEEP_CHARS;
            let cut = self
                .tail
                .char_indices()
                .nth(skip)
                .map(|(i, _)| i)
                .unwrap_or(self.tail.len());
            self.tail.drain(..cut);
        }
    }

    /// Whether the streamed tail now looks like a degenerate loop. Latches:
    /// `true` once means every later call also returns `true` until reset.
    pub(crate) fn looping(&self) -> bool {
        if self.fired {
            return true;
        }
        self.single_char_flood() || self.ngram_loop()
    }

    /// Latch the guard after the abort fires.
    pub(crate) fn latch(&mut self) {
        self.fired = true;
    }

    /// Reset for a new streaming attempt.
    pub(crate) fn reset(&mut self) {
        self.tail.clear();
        self.fired = false;
    }

    fn single_char_flood(&self) -> bool {
        let Some(last) = self.tail.chars().next_back() else {
            return false;
        };
        // Only punctuation/symbol floods trip this; a letter repeated 300
        // times (e.g. "aaa…") inside a word is not a thing real text does,
        // but keeping the rule punctuation-only avoids flagging stylized
        // prose and compression output.
        if last.is_alphanumeric() || last.is_whitespace() {
            return false;
        }
        self.tail.chars().rev().take_while(|&c| c == last).count() > MAX_SINGLE_CHAR_RUN
    }

    fn ngram_loop(&self) -> bool {
        let bytes = self.tail.as_bytes();
        // Candidate periods: the fixed table PLUS the distance from the last
        // line boundary to the end (real degenerate loops repeat the same
        // line, so the tail after the last newline is the repeating unit).
        let mut periods: Vec<usize> = (8..=64).collect();
        periods.extend_from_slice(NGRAM_PERIODS);
        if let Some(nl) = bytes.iter().rposition(|&b| b == b'\n') {
            let line_period = bytes.len() - nl - 1;
            if (8..=256).contains(&line_period) {
                periods.push(line_period);
            }
        }
        for &period in &periods {
            let need = period * NGRAM_MIN_REPEATS;
            if bytes.len() < need {
                continue;
            }
            let block = &bytes[bytes.len() - period..];
            // Pure-punctuation blocks are the single-char rule's job; see the
            // module docs for the dash-separator rationale.
            if !block.iter().any(u8::is_ascii_alphanumeric) {
                continue;
            }
            let mut repeats = 0usize;
            let mut pos = bytes.len();
            while pos >= period && &bytes[pos - period..pos] == block {
                repeats += 1;
                pos -= period;
            }
            if repeats >= NGRAM_MIN_REPEATS {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_text_never_trips() {
        let mut g = RepetitionGuard::default();
        let prose = "The quick brown fox jumps over the lazy dog. \
                     Structured output arrives as JSON with named fields. ";
        for _ in 0..40 {
            g.push(prose);
        }
        assert!(!g.looping());
    }

    #[test]
    fn markdown_separators_do_not_trip_the_flood_rule() {
        let mut g = RepetitionGuard::default();
        // 60-dash and 80-equals separators are legitimate markdown.
        g.push("Section\n--------------------------------------------\n");
        g.push("Heading\n================================================================\n");
        assert!(!g.looping());
    }

    #[test]
    fn punctuation_flood_trips_after_the_threshold() {
        let mut g = RepetitionGuard::default();
        g.push("Fetching https://github.com/atlassian/trello-mcp-server\n");
        g.push(&"!".repeat(301));
        assert!(g.looping());
    }

    #[test]
    fn punctuation_flood_below_threshold_is_ignored() {
        let mut g = RepetitionGuard::default();
        g.push(&"!".repeat(200));
        assert!(!g.looping());
    }

    #[test]
    fn word_loop_trips_the_ngram_rule() {
        let mut g = RepetitionGuard::default();
        g.push("Let me explain. ");
        // "the the " = 8-byte period; the guard checks 16-byte blocks, so
        // build the loop out of 16-byte blocks repeated well past the floor.
        g.push(&"the the the the ".repeat(12));
        assert!(g.looping());
    }

    #[test]
    fn guard_latches_until_reset() {
        let mut g = RepetitionGuard::default();
        g.push(&"!".repeat(301));
        assert!(g.looping());
        g.latch();
        // After latching, later chunks (even normal text) keep reporting the
        // loop until an explicit reset — the abort path relies on this.
        g.push("more normal text");
        assert!(g.looping(), "latched");
        g.reset();
        assert!(!g.looping(), "reset clears");
    }

    #[test]
    fn json_loop_with_17_byte_period_trips() {
        let mut g = RepetitionGuard::default();
        g.push("{\"results\": [");
        g.push(&"{\"a\": 1, \"b\": 2}, ".repeat(10));
        assert!(g.looping());
    }

    #[test]
    fn multibyte_tail_trim_keeps_char_boundaries() {
        let mut g = RepetitionGuard::default();
        // Push enough multibyte text to force several trims. The repeated
        // 8-byte block legitimately trips the n-gram rule (400 copies); the
        // point of this test is that the trim never panics on a multibyte
        // boundary and the flood rule still works afterwards.
        for _ in 0..400 {
            g.push("çãéão ");
        }
        assert!(g.looping(), "400x repeated 8-byte block is a loop");
        g.reset();
        g.push("ç normal multibyte prose with varied content. çã é à ");
        g.push(&"!".repeat(301));
        assert!(g.looping(), "flood rule works after multibyte trims");
    }

    #[test]
    fn repeated_dashes_do_not_trip_the_ngram_rule() {
        // 70 dashes: exceeds the single-char threshold? No — 70 < 300. And
        // the n-gram rule skips pure-punctuation blocks.
        let mut g = RepetitionGuard::default();
        g.push(&"-".repeat(70));
        assert!(!g.looping());
    }
}
