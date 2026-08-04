//! Canonical semantic cache for media-understanding results (plan section 11.1).
//!
//! The semantic key covers every variable that can change the meaning of a
//! delegate result: source digest, category, concrete provider/model,
//! transport/strategy, prompt fingerprint, schema fingerprint, instruction
//! fingerprint, and preprocess profile/version. Results are stored as
//! immutable BLAKE3-addressed objects under `objects/results/<key>.json` and
//! are **never keyed by result text** — keying by output text would serve stale
//! or reused semantics across providers, models, or prompts.

use crate::session::media::artifacts::MediaArtifactStore;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;
use xai_grok_tools::media::backend::MediaUnderstandingResult;
use xai_grok_tools::media::domain::{MediaCategory, MediaCategoryStrategy};

/// Canonical tuple that determines the meaning of a media-understanding
/// result. Two requests with equal keys must be semantically interchangeable;
/// two requests differing in any field must not share a cached result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SemanticCacheKey {
    /// BLAKE3 hex digest of the source bytes.
    pub source_digest: String,
    pub category: MediaCategory,
    /// Concrete provider identity (e.g. `"xai"`, `"openrouter"`).
    pub provider: String,
    /// Concrete model ID that produced the semantics.
    pub model: String,
    /// Transport/strategy used for the delegate call.
    pub strategy: MediaCategoryStrategy,
    /// Fingerprint of the full delegate prompt template/context.
    pub prompt_fingerprint: String,
    /// Fingerprint of the structured-output JSON schema.
    pub schema_fingerprint: String,
    /// Fingerprint of the user instruction text (empty-string fingerprint when
    /// no instruction is present).
    pub instruction_fingerprint: String,
    /// Preprocess profile name.
    pub preprocess_profile: String,
    /// Preprocess profile version.
    pub preprocess_version: u32,
}

impl SemanticCacheKey {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        source_digest: String,
        category: MediaCategory,
        provider: String,
        model: String,
        strategy: MediaCategoryStrategy,
        prompt_fingerprint: String,
        schema_fingerprint: String,
        instruction_fingerprint: String,
        preprocess_profile: String,
        preprocess_version: u32,
    ) -> Self {
        Self {
            source_digest,
            category,
            provider,
            model,
            strategy,
            prompt_fingerprint,
            schema_fingerprint,
            instruction_fingerprint,
            preprocess_profile,
            preprocess_version,
        }
    }

    /// Canonical BLAKE3 content address of this key. Field order is fixed so
    /// the same logical key always serializes to the same bytes and therefore
    /// the same address, independent of map ordering.
    pub(crate) fn canonical(&self) -> String {
        let canonical_json = serde_json::json!({
            "source_digest": self.source_digest,
            "category": self.category,
            "provider": self.provider,
            "model": self.model,
            "strategy": self.strategy,
            "prompt_fingerprint": self.prompt_fingerprint,
            "schema_fingerprint": self.schema_fingerprint,
            "instruction_fingerprint": self.instruction_fingerprint,
            "preprocess_profile": self.preprocess_profile,
            "preprocess_version": self.preprocess_version,
        });
        let canonical_bytes = serde_json::to_string(&canonical_json)
            .expect("canonical semantic key is always serializable")
            .into_bytes();
        blake3::hash(&canonical_bytes).to_hex().to_string()
    }
}

/// Store-backed semantic cache. `get` returns the stored result for a key
/// (replay-hit semantics); `insert` persists an immutable result object.
#[derive(Debug, Clone)]
pub(crate) struct SemanticCache {
    store: MediaArtifactStore,
}

impl SemanticCache {
    pub(crate) fn open(session_dir: &Path) -> io::Result<Self> {
        Ok(Self {
            store: MediaArtifactStore::open(session_dir)?,
        })
    }

    pub(crate) fn store(&self) -> &MediaArtifactStore {
        &self.store
    }

    /// Look up a cached result by its canonical semantic key.
    pub(crate) fn get(
        &self,
        key: &SemanticCacheKey,
    ) -> io::Result<Option<MediaUnderstandingResult>> {
        Ok(self
            .store
            .get_result(&key.canonical())?
            .map(|stored| stored.result))
    }

    /// Persist a result under its canonical semantic key.
    pub(crate) fn insert(
        &self,
        key: &SemanticCacheKey,
        result: &MediaUnderstandingResult,
    ) -> io::Result<()> {
        self.store.put_result(&key.canonical(), result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_tools::media::backend::{MediaProvenance, MediaSemantics};
    use xai_grok_tools::media::domain::MediaSource;

    fn base_key() -> SemanticCacheKey {
        SemanticCacheKey::new(
            "a".repeat(64),
            MediaCategory::Image,
            "xai".to_string(),
            "grok-4.5".to_string(),
            MediaCategoryStrategy::Native,
            "prompt-fp".to_string(),
            "schema-fp".to_string(),
            "instr-fp".to_string(),
            "default".to_string(),
            1,
        )
    }

    fn result(text: &str) -> MediaUnderstandingResult {
        MediaUnderstandingResult {
            results: vec![MediaSemantics {
                source: MediaSource::Path {
                    path: "assets/x.png".to_string(),
                },
                category: MediaCategory::Image,
                text: text.to_string(),
                provenance: MediaProvenance {
                    provider: "xai".to_string(),
                    model: "grok-4.5".to_string(),
                    strategy: MediaCategoryStrategy::Native,
                },
            }],
            attempts: vec![],
        }
    }

    #[test]
    fn media_cache_canonical_key_covers_every_variable() {
        let base = base_key();
        let canonical = base.canonical();
        assert_eq!(canonical.len(), 64);
        assert!(canonical.bytes().all(|b| b.is_ascii_hexdigit()));

        // Varying any single field must change the canonical address.
        let mutations: Vec<SemanticCacheKey> = vec![
            SemanticCacheKey::new(
                "b".repeat(64),
                base.category,
                base.provider.clone(),
                base.model.clone(),
                base.strategy,
                base.prompt_fingerprint.clone(),
                base.schema_fingerprint.clone(),
                base.instruction_fingerprint.clone(),
                base.preprocess_profile.clone(),
                base.preprocess_version,
            ),
            SemanticCacheKey::new(
                base.source_digest.clone(),
                MediaCategory::Audio,
                base.provider.clone(),
                base.model.clone(),
                base.strategy,
                base.prompt_fingerprint.clone(),
                base.schema_fingerprint.clone(),
                base.instruction_fingerprint.clone(),
                base.preprocess_profile.clone(),
                base.preprocess_version,
            ),
            SemanticCacheKey::new(
                base.source_digest.clone(),
                base.category,
                "openrouter".to_string(),
                base.model.clone(),
                base.strategy,
                base.prompt_fingerprint.clone(),
                base.schema_fingerprint.clone(),
                base.instruction_fingerprint.clone(),
                base.preprocess_profile.clone(),
                base.preprocess_version,
            ),
            SemanticCacheKey::new(
                base.source_digest.clone(),
                base.category,
                base.provider.clone(),
                "grok-vision".to_string(),
                base.strategy,
                base.prompt_fingerprint.clone(),
                base.schema_fingerprint.clone(),
                base.instruction_fingerprint.clone(),
                base.preprocess_profile.clone(),
                base.preprocess_version,
            ),
            SemanticCacheKey::new(
                base.source_digest.clone(),
                base.category,
                base.provider.clone(),
                base.model.clone(),
                MediaCategoryStrategy::Transcription,
                base.prompt_fingerprint.clone(),
                base.schema_fingerprint.clone(),
                base.instruction_fingerprint.clone(),
                base.preprocess_profile.clone(),
                base.preprocess_version,
            ),
            SemanticCacheKey::new(
                base.source_digest.clone(),
                base.category,
                base.provider.clone(),
                base.model.clone(),
                base.strategy,
                "other-prompt".to_string(),
                base.schema_fingerprint.clone(),
                base.instruction_fingerprint.clone(),
                base.preprocess_profile.clone(),
                base.preprocess_version,
            ),
            SemanticCacheKey::new(
                base.source_digest.clone(),
                base.category,
                base.provider.clone(),
                base.model.clone(),
                base.strategy,
                base.prompt_fingerprint.clone(),
                "other-schema".to_string(),
                base.instruction_fingerprint.clone(),
                base.preprocess_profile.clone(),
                base.preprocess_version,
            ),
            SemanticCacheKey::new(
                base.source_digest.clone(),
                base.category,
                base.provider.clone(),
                base.model.clone(),
                base.strategy,
                base.prompt_fingerprint.clone(),
                base.schema_fingerprint.clone(),
                "other-instruction".to_string(),
                base.preprocess_profile.clone(),
                base.preprocess_version,
            ),
            SemanticCacheKey::new(
                base.source_digest.clone(),
                base.category,
                base.provider.clone(),
                base.model.clone(),
                base.strategy,
                base.prompt_fingerprint.clone(),
                base.schema_fingerprint.clone(),
                base.instruction_fingerprint.clone(),
                "v2".to_string(),
                base.preprocess_version,
            ),
            SemanticCacheKey::new(
                base.source_digest.clone(),
                base.category,
                base.provider.clone(),
                base.model.clone(),
                base.strategy,
                base.prompt_fingerprint,
                base.schema_fingerprint,
                base.instruction_fingerprint,
                base.preprocess_profile,
                2,
            ),
        ];
        let mut seen = std::collections::BTreeSet::new();
        seen.insert(canonical);
        for mutated in mutations {
            let address = mutated.canonical();
            assert!(
                !seen.contains(&address),
                "key mutation must change the canonical address"
            );
            seen.insert(address);
        }
    }

    #[test]
    fn media_cache_insert_and_replay_hit() {
        let dir = tempfile::tempdir().unwrap();
        let cache = SemanticCache::open(dir.path()).unwrap();

        let key = base_key();
        assert!(cache.get(&key).unwrap().is_none(), "cold cache must miss");

        cache.insert(&key, &result("cached semantics")).unwrap();
        let hit = cache.get(&key).unwrap().expect("replay must hit");
        assert_eq!(hit.results[0].text, "cached semantics");
    }

    #[test]
    fn media_cache_never_keys_by_result_text() {
        let dir = tempfile::tempdir().unwrap();
        let cache = SemanticCache::open(dir.path()).unwrap();

        let key_a = base_key();
        let key_b = SemanticCacheKey::new(
            base_key().source_digest,
            MediaCategory::Image,
            "openrouter".to_string(),
            "grok-vision".to_string(),
            MediaCategoryStrategy::Auto,
            "prompt-b".to_string(),
            "schema-b".to_string(),
            "instr-b".to_string(),
            "default".to_string(),
            1,
        );

        // Identical result text under different keys must not alias.
        cache.insert(&key_a, &result("same text")).unwrap();
        cache.insert(&key_b, &result("same text")).unwrap();
        assert_ne!(key_a.canonical(), key_b.canonical());
        assert!(cache.get(&key_a).unwrap().is_some());
        assert!(cache.get(&key_b).unwrap().is_some());
    }

    #[test]
    fn media_cache_immutable_result_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let cache = SemanticCache::open(dir.path()).unwrap();

        let key = base_key();
        cache.insert(&key, &result("first")).unwrap();
        // Same key with different content: immutability keeps the first.
        cache.insert(&key, &result("second")).unwrap();
        assert_eq!(cache.get(&key).unwrap().unwrap().results[0].text, "first");
    }
}
