//! Shared bearer fragment semantics for authentication attribution.

/// Number of trailing Unicode scalar values allowed to cross an attribution
/// boundary.
pub const BEARER_TAIL_CHARS: usize = 12;

/// Return the final [`BEARER_TAIL_CHARS`] Unicode scalar values of `bearer`, or
/// the whole value when it is shorter.
///
/// The tail distinguishes JWT signatures and prefixed API keys whose heads are
/// commonly identical. Character indexing avoids slicing through UTF-8 input
/// supplied by configuration or an external credential provider.
pub fn bearer_tail(bearer: &str) -> &str {
    match bearer.char_indices().rev().nth(BEARER_TAIL_CHARS - 1) {
        Some((index, _)) => &bearer[index..],
        None => bearer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_distinguishing_jwt_tail() {
        assert_eq!(
            bearer_tail("eyJ0eXAiOiJKV1Q.shared-header.signature-tail"),
            "gnature-tail"
        );
    }

    #[test]
    fn counts_unicode_characters_instead_of_bytes() {
        assert_eq!(bearer_tail("aébcdefghijkl"), "ébcdefghijkl");
        assert_eq!(bearer_tail("ééééééééééééé"), "éééééééééééé");
        assert_eq!(bearer_tail("🔑🔑🔑🔑🔑🔑🔑"), "🔑🔑🔑🔑🔑🔑🔑");
    }

    #[test]
    fn preserves_short_exact_and_empty_values() {
        for value in ["abc", "123456789012", ""] {
            assert_eq!(bearer_tail(value), value);
        }
    }
}
