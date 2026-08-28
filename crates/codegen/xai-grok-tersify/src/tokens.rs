//! Token counting for compression ratios.
//!
//! Counts are local estimates. Provider usage is authoritative downstream, so
//! every number produced here must keep its basis name (`bpe-o200k` or
//! `approx-chars/4`) attached for display; an estimate without its basis reads
//! as a measurement and gets quoted as one.
//!
//! A real BPE tokenizer is the eventual default (matching the upstream engine's
//! `o200k_base` vocab so ratios compare across implementations). This revision
//! ships the deterministic approximation — the same fallback the upstream
//! counter degrades to — behind the same trait, and treats the swap as an
//! additive change: no compressor may know the difference.

use serde::{Deserialize, Serialize};

/// Estimates the token length of a payload.
///
/// Implementations must be deterministic: the same bytes always produce the
/// same count. A counter that drifts between calls silently rewrites history
/// in every ratio and cache key derived from it.
pub trait Counter: core::fmt::Debug {
    /// Local token estimate for `bytes`.
    fn count(&self, bytes: &[u8]) -> usize;

    /// Identifies the counting basis (e.g. `"bpe-o200k"`) for display and
    /// cross-surface parity.
    fn name(&self) -> &'static str;
}

/// Deterministic bytes/4 estimator, matching the upstream fallback basis.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApproxCounter;

impl Counter for ApproxCounter {
    fn count(&self, bytes: &[u8]) -> usize {
        approx_tokens(bytes)
    }

    fn name(&self) -> &'static str {
        "approx-chars/4"
    }
}

/// ~4 characters per token, counted in runes so multibyte text is not inflated
/// by its UTF-8 length. Minimum 1 for any non-empty payload.
#[must_use]
pub fn approx_tokens(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    let runes = String::from_utf8_lossy(bytes).chars().count();
    (runes / 4).max(1)
}

/// A token estimate together with the basis that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenEstimate {
    pub tokens: usize,
    /// Counting basis (`"bpe-o200k"`, `"approx-chars/4"`). Owned so the
    /// estimate can be deserialized from persisted rows.
    pub basis: String,
}

impl TokenEstimate {
    #[must_use]
    pub fn new(tokens: usize, basis: &str) -> Self {
        Self {
            tokens,
            basis: basis.to_string(),
        }
    }
}

/// The engine's shared default counter.
#[must_use]
pub fn default_counter() -> &'static dyn Counter {
    &ApproxCounter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_payload_counts_zero() {
        assert_eq!(approx_tokens(b""), 0);
    }

    #[test]
    fn short_payload_never_counts_zero() {
        // A two-character payload still costs the model at least one token;
        // reporting 0 would make a compressor claim it removed something that
        // cannot be measured.
        assert_eq!(approx_tokens(b"hi"), 1);
    }

    #[test]
    fn count_is_runes_not_utf8_bytes() {
        // "中" is three UTF-8 bytes but one character: 30 copies are 30 runes,
        // so the estimate is 30/4 = 7. Counting bytes would report 22.
        let payload = "中".repeat(30).into_bytes();
        assert_eq!(approx_tokens(&payload), 7);
    }

    #[test]
    fn estimate_carries_its_basis() {
        let counter = default_counter();
        let est = TokenEstimate::new(counter.count(b"hello world"), counter.name());
        assert_eq!(est.basis, "approx-chars/4");
    }

    #[test]
    fn same_bytes_always_count_the_same() {
        let counter = default_counter();
        let payload = b"the quick brown fox jumps over the lazy dog";
        assert_eq!(counter.count(payload), counter.count(payload));
    }
}
