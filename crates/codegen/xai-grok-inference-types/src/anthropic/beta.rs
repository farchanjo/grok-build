//! Typed, deduplicated Anthropic beta header set.

use std::collections::BTreeSet;
use std::fmt;

/// Beta header required for the Files API (`POST/GET/DELETE /v1/files`).
pub const FILES_API_BETA: &str = "files-api-2025-04-14";

/// A single Anthropic beta feature string (e.g. `files-api-2025-04-14`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AnthropicBeta(String);

impl AnthropicBeta {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn files_api() -> Self {
        Self::new(FILES_API_BETA)
    }
}

impl fmt::Debug for AnthropicBeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AnthropicBeta").field(&self.0).finish()
    }
}

impl fmt::Display for AnthropicBeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for AnthropicBeta {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for AnthropicBeta {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Explicit, deduplicated set of Anthropic beta headers.
///
/// Empty by default. Callers opt in per feature; there is no "all betas" mode.
/// Files methods add only [`FILES_API_BETA`] automatically.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct AnthropicBetaSet {
    inner: BTreeSet<AnthropicBeta>,
}

impl AnthropicBetaSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn insert(&mut self, beta: impl Into<AnthropicBeta>) -> bool {
        self.inner.insert(beta.into())
    }

    pub fn contains(&self, beta: &str) -> bool {
        self.inner.iter().any(|b| b.as_str() == beta)
    }

    /// Return a copy that always includes the Files API beta (idempotent).
    pub fn with_files_api(&self) -> Self {
        let mut out = self.clone();
        out.insert(AnthropicBeta::files_api());
        out
    }

    /// Header value for `anthropic-beta`: comma-separated, sorted, unique.
    /// Returns `None` when the set is empty (header omitted).
    pub fn header_value(&self) -> Option<String> {
        if self.inner.is_empty() {
            return None;
        }
        Some(
            self.inner
                .iter()
                .map(AnthropicBeta::as_str)
                .collect::<Vec<_>>()
                .join(","),
        )
    }

    pub fn iter(&self) -> impl Iterator<Item = &AnthropicBeta> {
        self.inner.iter()
    }
}

impl fmt::Debug for AnthropicBetaSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set()
            .entries(self.inner.iter().map(AnthropicBeta::as_str))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_by_default_and_no_header() {
        let set = AnthropicBetaSet::new();
        assert!(set.is_empty());
        assert_eq!(set.header_value(), None);
    }

    #[test]
    fn deduplicates_and_sorts() {
        let mut set = AnthropicBetaSet::new();
        set.insert("z-beta");
        set.insert("a-beta");
        set.insert("a-beta");
        assert_eq!(set.len(), 2);
        assert_eq!(set.header_value().as_deref(), Some("a-beta,z-beta"));
    }

    #[test]
    fn with_files_api_is_idempotent() {
        let set = AnthropicBetaSet::new().with_files_api().with_files_api();
        assert!(set.contains(FILES_API_BETA));
        assert_eq!(set.len(), 1);
        assert_eq!(set.header_value().as_deref(), Some(FILES_API_BETA));
    }
}
