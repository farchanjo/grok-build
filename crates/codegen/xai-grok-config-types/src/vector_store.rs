//! Remote vector-store mirror configuration (`[vector_stores.*]`).
//!
//! A named entry describes one remote vector store a memory or prime index
//! may mirror to. Only non-secret fields live here: the bearer token is
//! resolved at runtime from the vault (`milvus::<store-id>::token` in
//! `auth.json`) or the `MILVUS_TOKEN_FOR_<ID>` environment variable — never
//! from config.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// One named remote vector store (`[vector_stores.<id>]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VectorStoreConfig {
    /// Backend implementation. Currently only `milvus` is supported.
    pub backend: String,
    /// Server URI, e.g. `http://localhost:19530`.
    pub uri: String,
    /// Per-call timeout in seconds. Defaults to
    /// `xai_grok_memory::mirror::DEFAULT_MIRROR_TIMEOUT_SECS` and is
    /// floored at one second.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

/// Named vector-store table (`[vector_stores]`), keyed by store id.
///
/// Key order is preserved for deterministic resolution and comment-preserving
/// writes.
pub type VectorStoresConfig = IndexMap<String, VectorStoreConfig>;

impl VectorStoreConfig {
    /// Validate the fields the mirror resolver depends on. Returns a
    /// secret-free diagnostic on failure.
    pub fn validate(&self) -> Result<(), String> {
        if self.backend.trim().is_empty() {
            return Err("vector store `backend` must not be empty".into());
        }
        if self.backend.trim() != "milvus" {
            return Err(format!(
                "unsupported vector store backend `{}` (supported: milvus)",
                self.backend.trim()
            ));
        }
        let uri = self.uri.trim();
        if uri.is_empty() {
            return Err("vector store `uri` must not be empty".into());
        }
        if !uri.starts_with("http://") && !uri.starts_with("https://") {
            return Err("vector store `uri` must start with http:// or https://".into());
        }
        if uri.chars().any(|c| c.is_control() || c == '\0') {
            return Err("vector store `uri` must not contain control characters".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_store_config() {
        let config: VectorStoreConfig =
            toml::from_str("backend = \"milvus\"\nuri = \"http://localhost:19530\"").unwrap();
        assert_eq!(config.backend, "milvus");
        assert_eq!(config.uri, "http://localhost:19530");
        assert_eq!(config.timeout_secs, None);
        config.validate().unwrap();
    }

    #[test]
    fn parses_timeout_and_rejects_unknown_fields() {
        let config: VectorStoreConfig = toml::from_str(
            "backend = \"milvus\"\nuri = \"http://localhost:19530\"\ntimeout_secs = 5",
        )
        .unwrap();
        assert_eq!(config.timeout_secs, Some(5));
        assert!(
            toml::from_str::<VectorStoreConfig>(
                "backend = \"milvus\"\nuri = \"http://x\"\ntoken = \"leak\""
            )
            .is_err()
        );
    }

    #[test]
    fn validate_rejects_bad_backend_and_uri() {
        let mut config = VectorStoreConfig {
            backend: "qdrant".into(),
            uri: "http://localhost:19530".into(),
            timeout_secs: None,
        };
        assert!(config.validate().is_err());
        config.backend = "milvus".into();
        config.uri = "localhost:19530".into();
        assert!(config.validate().is_err());
        config.uri = "http://localhost:19530".into();
        config.validate().unwrap();
    }
}
