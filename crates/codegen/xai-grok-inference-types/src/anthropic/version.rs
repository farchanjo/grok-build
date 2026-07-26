//! Pinned Anthropic API version header value.
//!
//! Exactly one constant for the entire repository: every direct Anthropic
//! request must send `anthropic-version: 2023-06-01`.

/// Value of the required `anthropic-version` request header.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_version_is_pinned() {
        assert_eq!(ANTHROPIC_VERSION, "2023-06-01");
    }
}
