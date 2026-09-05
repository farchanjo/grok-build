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
//!
//! **Persisted metadata privacy:** the canonical payload stored in `meta`
//! carries `provider_instance_id` and `origin_host` — endpoint/account
//! identity labels, not credentials. They are persisted because they are
//! vector-compatibility determinants; `Debug`/telemetry render them as
//! identity strings only, and no credential/vector/text field is ever
//! persisted or rendered. Fingerprint field bytes are length-framed and
//! reject NUL/control characters, so crafted fields cannot collide.
//!
//! **The persisted payload is exactly the identity determinant set and
//! nothing more** (source spec + doc-prep + schema version). It never
//! contains query text, chunk text, vector values, provider API keys/tokens,
//! or debug/telemetry data; the identity labels it does carry are necessary
//! determinants — two accounts on the same host/model are different embedding
//! spaces — and are not secrets. See the schema docs and the
//! `payload_and_debug_expose_no_query_or_vectors` test.

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

/// Local unit-L2 normalization pin used by the Prime metadata index.
///
/// Distinct from [`NORMALIZATION_NONE`]: Prime always applies
/// [`crate::embedding::l2_normalize_v1`] after the provider returns and
/// fingerprints collections with this label so a route that claims
/// provider-side normalization cannot mix with Prime vectors.
pub const NORMALIZATION_L2_V1: &str = crate::embedding::L2_NORMALIZATION_VERSION;

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
/// Reject NUL and C0 control characters in identity strings: they would make
/// NUL-separated concatenation ambiguous and can hide in persisted labels.
pub(crate) fn validate_identity_field(value: &str) -> Result<(), String> {
    if value.chars().any(|c| (c as u32) < 0x20) {
        return Err("embedding identity field must not contain NUL/control characters".into());
    }
    Ok(())
}

impl EmbeddingSourceSpec {
    /// Compute the canonical vector fingerprint for this source spec and index config.
    pub fn fingerprint(
        &self,
        index_cfg: &xai_grok_config_types::MemoryIndexConfig,
    ) -> Result<VectorFingerprint, String> {
        VectorFingerprint::build(
            self.clone(),
            DocPreparationSpec::from_index_config(index_cfg),
            VECTOR_SCHEMA_VERSION,
        )
        .map(|(fp, _)| fp)
    }

    fn phys_bytes(&self) -> Vec<u8> {
        // Unambiguous length-framed encoding: every field is prefixed with its
        // byte length (u64 LE) so crafted fields (e.g. embedded NULs) can
        // never collide with a different field split. `build` validates the
        // fields first, so framing is defense-in-depth.
        let mut b: Vec<u8> = Vec::new();
        frame_str(&mut b, &self.provider_instance_id);
        frame_str(&mut b, self.incarnation.as_deref().unwrap_or(""));
        frame_str(&mut b, &self.origin_host);
        frame_str(&mut b, &self.embedding_path);
        frame_str(&mut b, &self.protocol);
        frame_str(&mut b, &self.model);
        b.extend_from_slice(&self.dimensions.to_le_bytes());
        b.push(0);
        frame_str(&mut b, &self.encoding);
        frame_str(&mut b, &self.normalization);
        b
    }
}

fn frame_str(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
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
        frame_str(&mut b, &self.version);
        frame_str(&mut b, &self.chunker);
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
    pub fn build(
        source: EmbeddingSourceSpec,
        document_preparation: DocPreparationSpec,
        vector_schema_version: u32,
    ) -> Result<(Self, String), String> {
        // Reject NUL/control characters in identity fields so the length-
        // framed encoding is never ambiguous and crafted fields cannot
        // collide with a different field split.
        validate_identity_field(&source.provider_instance_id)?;
        if let Some(inc) = &source.incarnation {
            validate_identity_field(inc)?;
        }
        validate_identity_field(&source.origin_host)?;
        validate_identity_field(&source.embedding_path)?;
        validate_identity_field(&source.protocol)?;
        validate_identity_field(&source.model)?;
        validate_identity_field(&source.encoding)?;
        validate_identity_field(&source.normalization)?;
        validate_identity_field(&document_preparation.version)?;
        validate_identity_field(&document_preparation.chunker)?;

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

    // -----------------------------------------------------------------------
    // Length-framed encoding rejects ambiguous and control-containing inputs.
    // -----------------------------------------------------------------------

    #[test]
    fn fingerprint_rejects_control_chars_in_identity_fields() {
        let setters: Vec<fn(String) -> EmbeddingSourceSpec> = vec![
            |s| EmbeddingSourceSpec {
                provider_instance_id: s,
                ..spec("m")
            },
            |s| EmbeddingSourceSpec {
                model: s,
                ..spec("m")
            },
            |s| EmbeddingSourceSpec {
                origin_host: s,
                ..spec("m")
            },
            |s| EmbeddingSourceSpec {
                embedding_path: s,
                ..spec("m")
            },
            |s| EmbeddingSourceSpec {
                encoding: s,
                ..spec("m")
            },
        ];
        for make in setters {
            let s = make("ab\u{0}c".into());
            assert!(
                VectorFingerprint::build(
                    s,
                    DocPreparationSpec::from_index_config(&MemoryIndexConfig::default()),
                    VECTOR_SCHEMA_VERSION,
                )
                .is_err(),
                "NUL/control chars must be rejected in identity fields"
            );
        }
    }

    #[test]
    fn fingerprint_framing_is_unambiguous() {
        // Field splits that would collide under NUL-separated concatenation
        // hash differently under the length-framed encoding.
        let a = build(&EmbeddingSourceSpec {
            provider_instance_id: "ab".into(),
            model: "c".into(),
            ..spec("m")
        });
        let b = build(&EmbeddingSourceSpec {
            provider_instance_id: "a".into(),
            model: "bc".into(),
            ..spec("m")
        });
        assert_ne!(
            a.hash, b.hash,
            "length-framed encoding must distinguish splits"
        );
        // Deterministic for identical inputs.
        assert_eq!(a.hash, build(&a.source).hash);
    }

    /// Carried privacy note: the persisted payload and every Debug/Display
    /// rendering expose the necessary identity labels (provider/host) but
    /// never query text, chunk text, vector values, or credential material.
    #[test]
    fn payload_and_debug_expose_no_query_or_vectors() {
        let s = EmbeddingSourceSpec {
            provider_instance_id: "acct-b".into(),
            incarnation: Some("inc-2".into()),
            origin_host: "embed.provider.example".into(),
            embedding_path: "/v1/embeddings".into(),
            protocol: "openai_compatible".into(),
            model: "text-embed-3-small".into(),
            dimensions: 1536,
            encoding: "float".into(),
            normalization: NORMALIZATION_NONE.into(),
        };
        let (fp, payload) = VectorFingerprint::build(
            s,
            DocPreparationSpec::from_index_config(&MemoryIndexConfig::default()),
            VECTOR_SCHEMA_VERSION,
        )
        .unwrap();

        // Necessary identity labels ARE persisted (identity determinants).
        assert!(
            payload.contains("acct-b"),
            "provider instance id is identity"
        );
        assert!(
            payload.contains("embed.provider.example"),
            "origin host is identity"
        );

        // Nothing non-identity is ever persisted or rendered: query/chunk
        // text, vector floats, and credential markers.
        let haystacks = [payload.as_str(), &format!("{fp:?}"), &format!("{fp}")];
        for forbidden in [
            "secret-query-text-x9",
            "Rust borrow checker",
            "0.39215687",
            "sk-",
            "api_key",
            "authorization",
            "Bearer",
            "secret-token",
        ] {
            for hay in &haystacks {
                assert!(
                    !hay.to_lowercase().contains(&forbidden.to_lowercase()),
                    "identity output must never expose {forbidden:?} (got {hay})"
                );
            }
        }
        // The source spec Display renders identity only.
        let disp = format!("{}", fp.source);
        assert!(disp.contains("acct-b"));
        assert!(!disp.contains("secret-token"));
    }
}
