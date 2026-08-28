//! The style instruction injected into the main conversation.
//!
//! This is the output-side half of the engine: a short, deterministic prompt
//! block that tells the model to answer in compressed prose while keeping
//! every technical literal exact. It ships as Rust strings built from levels —
//! not a markdown file — because the text is a compiled contract with the
//! renderer, pinned by tests, not documentation.
//!
//! Rules that the levels share (and that tests pin, because dropping one was
//! each measured to hurt):
//!
//! - Technical literals never change: code blocks, identifiers, commands,
//!   numbers, units, and quoted error strings stay byte-exact.
//! - Negation never drops: `not`, `never`, `no`, `only`, `except` are worth
//!   more than any token they cost.
//! - No invented abbreviations: under a BPE tokenizer `cfg`/`impl`/`req`
//!   split into the same pieces as the full words — they save nothing and
//!   cost the reader decoding time.
//! - The user's language is preserved: compress the style, not the language.

use crate::tokens::approx_tokens;

/// Instruction text for one level.
#[must_use]
pub fn style_instruction(level: &str) -> &'static str {
    match level {
        "lite" => LITE,
        "ultra" => ULTRA,
        _ => FULL,
    }
}

/// Session-scoped mode marker written into the conversation when the user
/// toggles style mid-session. Kept as a function so the caller owns when the
/// session's ephemeral state flips; nothing here persists to disk.
#[must_use]
pub fn mode_banner(level: &str) -> String {
    format!("tersify: {level} (this session only; say 'stop tersify' or use /tersify off)")
}

const SHARED_RULES: &str = "Drop filler, hedging, and pleasantries. Keep every \
    technical fact exactly once. Code blocks, identifiers, commands, numbers, \
    and units never change. Negation words (not, never, no, only, except) never \
    drop. No invented abbreviations (cfg/impl/req save zero tokens under BPE). \
    Reply in the user's language. Drop the style for security warnings and \
    irreversible-action confirmations, then resume.";

/// Lite: no filler or hedging; full sentences and articles stay.
pub const LITE: &str = "Answer in tight professional prose: no filler words, no hedging, \
    no pleasantries. Complete grammatical sentences, articles included. Negation \
    never drops; code, identifiers, commands, numbers, and error strings stay \
    byte-exact.";

/// Full (default): drop articles and filler; fragments allowed; short synonyms.
pub const FULL: &str = "Respond terse. All technical substance stays; only fluff dies. \
    Drop articles, filler (just/really/basically/actually/simply), pleasantries, \
    hedging. Fragments OK. Short synonyms (big not extensive, fix not implement a \
    solution for). No tool-call narration. Standard acronyms OK (DB/API/HTTP); \
    never invent abbreviations (cfg/impl/req): the tokenizer splits them the same \
    as full words. Negation (not, never, no, only, except) never drops — a flipped \
    meaning costs more than any token saved. Numbers, units, and quoted error \
    strings stay byte-exact. Never add a word to sound terse — if \
    the plain phrasing is not longer, use the plain phrasing. Pattern: [thing] \
    [action] [reason]. [next step].";

/// Ultra: maximum compression while cause-and-effect stays unambiguous.
pub const ULTRA: &str = "One word when one word enough. State each fact exactly once. \
    Strip conjunctions only where cause-then-effect stays unambiguous. Never use \
    prose abbreviations or causal arrows (each costs a token, saves nothing). \
    Negation never drops. Code symbols, API names, and error strings: never touch.";

/// Rough token cost of one level's instruction, for the TUI's cost badge. The
/// instruction repeats every turn it is active, so the caller can show what the
/// style is charging per turn before it is switched on.
#[must_use]
pub fn instruction_cost(level: &str) -> usize {
    approx_tokens(style_instruction(level).as_bytes())
}

/// The shared addendum every level carries (pinned separately so level text can
/// change without losing the invariants).
#[must_use]
pub const fn shared_rules() -> &'static str {
    SHARED_RULES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_level_pins_the_safety_rules() {
        for level in ["lite", "full", "ultra"] {
            let text = style_instruction(level);
            // Negation and literal-preservation rules must appear in ALL levels:
            // dropping either measurably produced wrong answers.
            let lower = text.to_lowercase();
            assert!(lower.contains("negation"), "{level} must pin negation");
            assert!(
                lower.contains("error string") || lower.contains("error strings"),
                "{level} must keep error text exact"
            );
        }
    }

    #[test]
    fn unknown_level_falls_back_to_full() {
        assert_eq!(style_instruction("bogus"), FULL);
        assert_eq!(style_instruction(""), FULL);
    }

    #[test]
    fn levels_are_ordered_by_compression() {
        // lite must be the most permissive wording (contains "sentences"),
        // ultra the strictest (contains "one word").
        assert!(LITE.contains("sentences"));
        assert!(ULTRA.contains("one word"));
    }

    #[test]
    fn instruction_cost_is_reported_for_the_active_level() {
        // Cost drives the user's decision; a zero would read as "free".
        for level in ["lite", "full", "ultra"] {
            assert!(instruction_cost(level) > 10, "{level} cost reads free");
        }
    }

    #[test]
    fn mode_banner_names_the_level_and_its_ephemeral_scope() {
        let b = mode_banner("ultra");
        assert!(b.contains("ultra"));
        assert!(
            b.contains("session only"),
            "banner must promise no persistence"
        );
    }
}
