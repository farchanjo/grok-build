//! Durable non-secret vector identity for memory indexes.
//!
//! PR21 pins the embedding source used to build a memory index's vectors so
//! that a provider reload, dimensional drift, or transient failure can never
//! silently switch the embedding space or mix vectors from more than one model
//! in an index/session.
//!
//! [`EmbeddingSourceSpec`] is a **credential-free, provider-agnostic** data
//! record describing only the non-secret determinants of vector
//! compatibility: provider instance id, persistent incarnation, normalized
//! origin + embedding path, protocol, upstream model, dimensions, encoding,
//! and normalization. It intentionally carries **no** API keys, bearer
//! tokens, OAuth material, or other credentials.
//!
//! [`VectorFingerprint`] is the versioned canonical payload persisted along
//! side a complete installed vector set. It includes the source spec, the
//! document/chunk-preparation parameters/version, and the vector/schema
//! format version. Two valid indexes share vectors only when their canonical
//! fingerprints match; a change to any included field forces a rebuild.
//!
//! **Excluded from identity:** credential generation/rotation, secrets,
//! request text, the vectors themselves, and reranker-only / ranking settings
//! (reranker config, MMR, source weights, decay, thresholds). Credential
//! rotation and reranker-only changes therefore never trigger a rebuild.

use blake3;

use xai_grok_config_types::MemoryIndexConfig;

/// Canonical fingerprint format version. Bump when the serialized payload
/// layout changes in a way that must invalidate previously persisted
/// fingerprints (forces a compatibility rebuild).
pub const FINGERPRINT_FORMAT_VERSION: u32 = 1;

/// Embedding schema compatibility version persisted with the vector set.
///
/// Bump when the vector storage schema semantics change such that previously
/// built vectors are no longer usable (e.g., normalization change, vector
/// format change). This is not the same as [`super::schema::SCHEMA_VERSION`],
/// which gates the whole index schema.
pub const VECTOR_SCHEMA_VERSION: u32 = 1;

/// Internal digest length used for the persisted fingerprint hash.
const HASH_LEN: usize = 16;

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

/// Deterministic normalization label for the embedding source.
///
/// `"none"` mirrors the runtime pin used by the retrieval graph; embeddings
/// are expected pre-normalized to unit L2 norm by the provider/pipeline.
pub const NORMALIZATION_NONE: &str = "none";

/// Deterministic document-preparation version for the canonical chunker.
///
/// Bump when `chunk_markdown` semantically changes the produced chunks for
/// the same inputs, forcing a vector rebuild (vectors were computed over the
/// prior chunk texts).
pub const DOC_PREP_VERSION: &str = "v0";

/// The chunker/algorithm label that produced the document chunks.
pub const CHUNKER_ID: &str = "markdown";

// ---------------------------------------------------------------------------
// Embedding source spec (credential-free)
// ---------------------------------------------------------------------------

/// Non-secret, provider-agnostic description of the embedding route that
/// produced a memory index's vectors. Used as the basis of the canonical
/// vector fingerprint. Contains **no credentials**.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EmbeddingSourceSpec {
    /// Exact provider-instance id (`[model_providers.<id>]` or built-in).
    pub provider_instance_id: String,
    /// Persistent incarnation label for the provider instance. Changes only
    /// on provider rotation — not on transient reloads. `None` when the
    /// source does not track incarnations (legacy synthesis).
    pub incarnation: Option<String>,
    /// Normalized origin host (no credentials, no full URL, port preserved).
    pub origin_host: String,
    /// Normalized embedding endpoint path (e.g. `/v1/embeddings`).
    pub embedding_path: String,
    /// Wire protocol (`"openai_compatible"`).
    pub protocol: String,
    /// Upstream model slug.
    pub model: String,
    /// Fixed embedding dimensions.
    pub dimensions: usize,
    /// Vector encoding (`"float"` or `"base64"`).
    pub encoding: String,
    /// Embedding normalization (`"none"`).
    pub normalization: String,
}

impl std::fmt::Display for EmbeddingSourceSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}/{}@{}:{}:{}/{}d/{}",
            self.provider_instance_id,
            self.model,
            self.origin_host,
            self.embedding_path,
            self.protocol,
            self.dimensions,
            self.encoding
        )
    }
}

/// Deterministic raw bytes that feed the canonical digest for a source spec.
impl EmbeddingSourceSpec {
    fn phys_bytes(&self) -> Vec<u8> {
        let mut b: Vec<u8> = Vec::new();
        b.extend_from_slice(self.provider_instance_id.as_bytes());
        b.push(0);
        b.extend_from_slice(self.incarnation.as_deref().unwrap_or("").as_bytes());
        b.push(0);
        b.extend_from_slice(self.origin_host.as_bytes());
        b.push(0);
        b.extend_from_slice(self.embedding_path.as_bytes());
        b.push(0);
        b.extend_from_slice(self.protocol.as_bytes());
        b.push(0);
        b.extend_from_slice(self.model.as_bytes());
        b.push(0);
        b.extend_from_slice(&self.dimensions.to_le_bytes());
        b.push(0);
        b.extend_from_slice(self.encoding.as_bytes());
        b.push(0);
        b.extend_from_slice(self.normalization.as_bytes());
        b
    }
}

// ---------------------------------------------------------------------------
// Document / chunk preparation
// ---------------------------------------------------------------------------

/// Document / chunk-preparation parameters that interact with embeddings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocPreparationSpec {
    /// Marker for the canonical chunking algorithm+version.
    pub version: String,
    /// Chunker algorithm id.
    pub chunker: String,
    /// `max_chunk_chars` used when chunking (embedding input borders).
    pub max_chunk_chars: usize,
    /// `chunk_overlap_chars` used when chunking.
    pub chunk_overlap_chars: usize,
}

impl DocPreparationSpec {
    /// Derive the doc-prep spec from the configured index/chunk settings.
    pub fn from_index_config(cfg: &MemoryIndexConfig) -> Self {
        Self {
            version: DOC_PREP_VERSION.to_owned(),
            chunker: CHUNKER_ID.to_owned(),
            max_chunk_chars: cfg.max_chunk_chars,
            chunk_overlap_chars: cfg.chunk_overlap_chars,
        }
    }
}

impl DocPreparationSpec {
    fn bytes(&self) -> Vec<u8> {
        let mut b: Vec<u8> = Vec::new();
        b.extend_from_slice(self.version.as_bytes());
        b.push(0);
        b.extend_from_slice(self.chunker.as_bytes());
        b.push(0);
        b.extend_from_slice(&self.max_chunk_chars.to_le_bytes());
        b.extend_from_slice(&self.chunk_overlap_chars.to_le_bytes());
        b
    }
}

/// Serialized canonical payload for diagnostics + persisted metadata.
fn payload_json(
    source: &EmbeddingSourceSpec,
    doc: &DocPreparationSpec,
    vector_schema_version: u32,
) -> String {
    format!(
        r#"{{"version":{},"source":{{"provider_instance_id":{},"incarnation":{},"origin_host":{},"embedding_path":{},"protocol":{},"model":{},"dimensions":{},"encoding":{},"normalization":{}}},"document_preparation":{{"version":{},"chunker":{},"max_chunk_chars":{},"chunk_overlap_chars":{}}},"vector_schema_version":{}}}"#,
        FINGERPRINT_FORMAT_VERSION,
        json_str(&source.provider_instance_id),
        json_str(source.incarnation.as_deref().unwrap_or("null")),
        json_str(&source.origin_host),
        json_str(&source.embedding_path),
        json_str(&source.protocol),
        json_str(&source.model),
        source.dimensions,
        json_str(&source.encoding),
        json_str(&source.normalization),
        json_str(&doc.version),
        json_str(&doc.chunker),
        doc.max_chunk_chars,
        doc.chunk_overlap_chars,
        vector_schema_version,
    )
}

/// Escape a string as a JSON string literal.
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Versioned canonical vector-fingerprint.
///
/// Persisted next to a complete, compatible vector set. Same fingerprint ⇒
/// vectors are reusable; any field change (other than credentials or
/// reranker-only setting) ⇒ rebuild required. Never contains vectors,
/// request text, or secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorFingerprint {
    /// Canonical payload format version ([`FINGERPRINT_FORMAT_VERSION`]).
    pub version: u32,
    /// Credential-free embedding source.
    pub source: EmbeddingSourceSpec,
    /// Document/chunk preparation parameters.
    pub document_preparation: DocPreparationSpec,
    /// Vector storage schema compatibility version.
    pub vector_schema_version: u32,
    /// Short hex digest of the canonical payload.
    pub hash: String,
}

impl VectorFingerprint {
    /// Compute the canonical payload and its digest (blake3). Returns
    /// `(fingerprint, persisted_payload_json)`.
    pub(crate) fn build(
        source: EmbeddingSourceSpec,
        document_preparation: DocPreparationSpec,
        vector_schema_version: u32,
    ) -> Result<(Self, String), String> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"memvec/fp/v1\0");
        hasher.update(&FINGERPRINT_FORMAT_VERSION.to_le_bytes());
        hasher.update(b"\0");
        hasher.update(&source.phys_bytes());
        hasher.update(b"\0");
        hasher.update(&document_preparation.bytes());
        hasher.update(b"\0");
        hasher.update(&vector_schema_version.to_le_bytes());
        let hash = hex_encode(&hasher.finalize().as_bytes()[..HASH_LEN]);

        let payload = payload_json(&source, &document_preparation, vector_schema_version);

        Ok((
            VectorFingerprint {
                version: FINGERPRINT_FORMAT_VERSION,
                source,
                document_preparation,
                vector_schema_version,
                hash,
            },
            payload,
        ))
    }

    /// Equality over all canonical determinants except the derived hash.
    pub fn determinants_match(&self, other: &Self) -> bool {
        self.version == other.version
            && self.source == other.source
            && self.document_preparation == other.document_preparation
            && self.vector_schema_version == other.vector_schema_version
    }

    /// The persisted short hash used for fast equality.
    pub fn hash(&self) -> &str {
        &self.hash
    }
}

impl std::fmt::Display for VectorFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "vecfp/{}:{}", self.version, self.hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_config_types::MemoryIndexConfig;

    fn spec(model: &str) -> EmbeddingSourceSpec {
        EmbeddingSourceSpec {
            provider_instance_id: "acct-a".into(),
            incarnation: Some("inc-1".into()),
            origin_host: "api.example.com".into(),
            embedding_path: "/v1/embeddings".into(),
            protocol: "openai_compatible".into(),
            model: model.into(),
            dimensions: 1536,
            encoding: "float".into(),
            normalization: NORMALIZATION_NONE.into(),
        }
    }

    fn build(s: &EmbeddingSourceSpec) -> VectorFingerprint {
        VectorFingerprint::build(
            s.clone(),
            DocPreparationSpec::from_index_config(&MemoryIndexConfig::default()),
            VECTOR_SCHEMA_VERSION,
        )
        .unwrap()
        .0
    }

    fn doc_prep(max_chunk_chars: usize) -> DocPreparationSpec {
        DocPreparationSpec {
            version: DOC_PREP_VERSION.into(),
            chunker: CHUNKER_ID.into(),
            max_chunk_chars,
            chunk_overlap_chars: 320,
        }
    }

    #[test]
    fn same_spec_same_hash() {
        assert_eq!(build(&spec("m")).hash, build(&spec("m")).hash);
    }

    #[test]
    fn model_change_changes_hash() {
        assert_ne!(build(&spec("a")).hash, build(&spec("b")).hash);
    }

    #[test]
    fn endpoint_change_changes_hash() {
        let a = spec("m");
        let mut b = a.clone();
        b.origin_host = "other.example.net".into();
        assert_ne!(build(&a).hash, build(&b).hash);
    }

    #[test]
    fn dimensions_change_changes_hash() {
        let a = spec("m");
        let mut b = a.clone();
        b.dimensions = 1024;
        assert_ne!(build(&a).hash, build(&b).hash);
    }

    #[test]
    fn doc_prep_change_changes_hash() {
        let a = VectorFingerprint::build(spec("m"), doc_prep(1600), VECTOR_SCHEMA_VERSION)
            .unwrap()
            .0;
        let b = VectorFingerprint::build(spec("m"), doc_prep(2000), VECTOR_SCHEMA_VERSION)
            .unwrap()
            .0;
        assert_ne!(a.hash, b.hash, "chunk prep params must be part of identity");
    }

    #[test]
    fn vector_schema_version_change_changes_hash() {
        let a = VectorFingerprint::build(
            spec("m"),
            DocPreparationSpec::from_index_config(&MemoryIndexConfig::default()),
            VECTOR_SCHEMA_VERSION,
        )
        .unwrap()
        .0;
        let b = VectorFingerprint::build(
            spec("m"),
            DocPreparationSpec::from_index_config(&MemoryIndexConfig::default()),
            VECTOR_SCHEMA_VERSION + 1,
        )
        .unwrap()
        .0;
        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn reranker_only_and_credential_changes_do_not_alter_identity() {
        // Specs carry no reranker and no credential fields: rotating either
        // leaves the fingerprint untouched.
        assert_eq!(build(&spec("m")).hash, build(&spec("m")).hash);
        // Determinants_match includes every persisted field (rerankers and
        // credentials are not among them).
        let a = build(&spec("m"));
        let b = build(&spec("m"));
        assert!(a.determinants_match(&b));
    }

    #[test]
    fn debug_omits_vectors_and_secrets() {
        let fp = build(&spec("m"));
        let dbg = format!("{fp:?}");
        assert!(
            dbg.contains("hash"),
            "Debug must expose the digest for diagnostics"
        );
        assert!(!dbg.contains("sk-"), "Debug must never render credentials");
        assert!(
            !dbg.contains("0.39215687"),
            "Debug must never render vector floats"
        );
        let disp = format!("{fp}");
        assert!(disp.contains("vecfp/"));
    }

    #[test]
    fn payload_json_parseable_and_credential_free() {
        let (_, payload) = VectorFingerprint::build(
            spec("m"),
            DocPreparationSpec::from_index_config(&MemoryIndexConfig::default()),
            VECTOR_SCHEMA_VERSION,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(v["source"]["model"], "m");
        assert_eq!(v["source"]["dimensions"], 1536);
        assert!(payload.contains("api.example.com"));
        assert!(!payload.contains("sk-"));
    }
}
