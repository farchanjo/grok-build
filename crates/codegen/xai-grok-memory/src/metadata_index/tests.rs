use std::sync::Arc;

use tempfile::TempDir;
use tokio_util::sync::CancellationToken;
use xai_sqlite_journal::JournalMode;

use super::rebuild::{
    CLAIM_PRE_COMMIT_FAIL, CLEAR_PRE_COMMIT_FAIL, DISCARD_PRE_COMMIT_FAIL, INSTALL_PRE_COMMIT_FAIL,
    claim_is_reclaimable, claim_owned_by_pid, clear_completed_target, commit_staged_vectors,
    compatible_readiness, discard_collection_rebuild, ensure_collection_vectors_ready,
    ensure_pending, install_vectors, pending_marker_present, stage_collection_vectors,
    stage_vector, staged_count, staging_complete, try_claim_rebuild,
};
use super::{
    CollectionKind, MetadataIndex, MetadataIndexError, MetadataItem, SCHEMA_VERSION,
    UPSERT_BEFORE_TXN_PAUSE, UPSERT_BEFORE_TXN_REACHED, metadata_index_path,
    metadata_index_path_for_cwd,
};
use crate::embedding::{EmbeddingProvider, MockEmbeddingProvider};
use crate::fingerprint::{
    EmbeddingSourceSpec, NORMALIZATION_L2_V1, NORMALIZATION_NONE, VECTOR_SCHEMA_VERSION,
    VectorFingerprint,
};
use crate::index::init_sqlite_vec;
use crate::rebuild::VectorReadiness;
use crate::workspace_identity::workspace_storage_identity;

fn spec(model: &str, dims: usize) -> EmbeddingSourceSpec {
    EmbeddingSourceSpec {
        provider_instance_id: "prime-meta".into(),
        incarnation: Some("inc-1".into()),
        origin_host: "embed.example.test".into(),
        embedding_path: "/v1/embeddings".into(),
        protocol: "openai_compatible".into(),
        model: model.into(),
        dimensions: dims,
        encoding: "float".into(),
        normalization: NORMALIZATION_NONE.into(),
    }
}

fn item(id: &str, name: &str, description: &str) -> MetadataItem {
    MetadataItem::new(id, name, description, "").unwrap()
}

fn extra_item(id: &str, name: &str, description: &str, extra: &str) -> MetadataItem {
    MetadataItem::new(id, name, description, extra).unwrap()
}

fn open(tmp: &TempDir) -> (std::path::PathBuf, MetadataIndex) {
    init_sqlite_vec();
    let db_path = tmp.path().join("indexes/prime/ws/metadata.sqlite");
    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    (db_path, idx)
}

fn fp_for(collection: CollectionKind, source: &EmbeddingSourceSpec) -> (VectorFingerprint, String) {
    VectorFingerprint::build(
        source.clone(),
        super::metadata_doc_prep(collection),
        VECTOR_SCHEMA_VERSION,
    )
    .unwrap()
}

fn journal_mode(idx: &MetadataIndex) -> String {
    idx.db()
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn schema_version_is_persisted() {
    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    assert_eq!(idx.schema_version(), SCHEMA_VERSION);
    assert!(idx.writable());
    let skills = idx.collection_state(CollectionKind::Skills).unwrap();
    let agents = idx
        .collection_state(CollectionKind::CallableAgents)
        .unwrap();
    assert_eq!(skills.item_count, 0);
    assert_eq!(agents.item_count, 0);
}

#[tokio::test]
async fn newer_schema_fails_closed_without_writes() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust sources")],
    )
    .unwrap();
    idx.db()
        .execute(
            "UPDATE meta SET value = ?1 WHERE key = 'schema_version'",
            rusqlite::params![(SCHEMA_VERSION + 100).to_string()],
        )
        .unwrap();
    drop(idx);

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    assert!(!idx.writable());
    assert!(!idx.vec_available());
    assert_eq!(idx.item_count(CollectionKind::Skills), 1);
    let err = idx
        .replace_inventory(CollectionKind::Skills, 2, &[])
        .unwrap_err();
    assert!(matches!(err, MetadataIndexError::ReadOnly));
    assert_eq!(idx.item_count(CollectionKind::Skills), 1);
    drop(idx);

    let out = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        None,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(out, VectorReadiness::Disabled),
        "newer-schema must not report Pending: {out:?}"
    );
    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    assert_eq!(idx.item_count(CollectionKind::Skills), 1);
}

fn assert_privacy(result: Result<MetadataItem, MetadataIndexError>) {
    assert!(
        matches!(result, Err(MetadataIndexError::Privacy(_))),
        "expected Privacy, got {result:?}"
    );
}

#[test]
fn privacy_rejects_body_prompt_credential_and_absolute_paths() {
    assert!(MetadataItem::new("a", "ok", "desc", r#"{"body":"secret"}"#).is_err());
    assert!(MetadataItem::new("a", "ok", "desc", r#"{"prompt":"do it"}"#).is_err());
    assert!(MetadataItem::new("a", "ok", "desc", r#"{"credential":"sk-test"}"#).is_err());
    assert!(MetadataItem::new("a", "ok", "desc", r#"{"paths":["/etc/passwd"]}"#).is_err());
    assert!(MetadataItem::new("a", "/tmp/skill", "desc", "").is_err());
    assert!(MetadataItem::new("not/id", "ok", "desc", "").is_err());
    assert!(
        extra_item(
            "ok",
            "lint",
            "lints rust",
            r#"{"when_to_use":"when linting","scope":"project","paths":["src/**"]}"#
        )
        .item_id
            == "ok"
    );
}

#[test]
fn privacy_rejects_embedded_and_unc_absolute_paths() {
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"/Users/foo/.ssh/id_rsa"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"~/projects/lint"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"\\Users\\foo\\skill"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"\\\\nas\\share\\skill"}"#,
    ));
    assert_privacy(MetadataItem::new("a", "uses /Users/foo tools", "desc", ""));
    assert_privacy(MetadataItem::new("a", "ok", "see /home/ubuntu notes", ""));
    assert_privacy(MetadataItem::new("a", "ok", "from ~/secret", ""));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"scope":"/Users/me"}"#,
    ));
    assert_privacy(MetadataItem::new("a", "ok", "see /etc/passwd", ""));
    assert_privacy(MetadataItem::new("a", "ok", r"see C:\Windows\system32", ""));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"use /usr/bin/git"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"see file:///etc/passwd"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"see file://localhost/etc/passwd"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"see FILE:/etc/passwd"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"see File://LOCALHOST/etc/passwd"}"#,
    ));
    assert_privacy(MetadataItem::new("a", "ok", "see file:///etc/passwd", ""));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "see file://localhost/etc/passwd",
        "",
    ));
    assert_privacy(MetadataItem::new("a", "ok", "see file:/etc/passwd", ""));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"file://localhost/etc/passwd"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"see file:%2f%2f%2fetc/passwd"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"see file:%2flocalhost%2fetc%2fpasswd"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "see file:./../../etc/passwd",
        "",
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"file:../etc/passwd"}"#,
    ));
    assert_privacy(MetadataItem::new("a", "ok", r"see file:\///etc/passwd", ""));
    assert!(
        extra_item(
            "ok",
            "lint",
            "lints rust",
            r#"{"when_to_use":"when linting","scope":"project","paths":["src/**"]}"#
        )
        .item_id
            == "ok",
        "relative path labels must still persist"
    );
    assert!(
        extra_item(
            "rel",
            "lint",
            "lints rust",
            r#"{"when_to_use":"see file:./notes.md","scope":"project","paths":["src/**"]}"#
        )
        .item_id
            == "rel",
        "relative file:./ URLs must still persist"
    );
    assert!(
        extra_item(
            "web",
            "lint",
            "lints rust",
            r#"{"when_to_use":"see https://example.com/docs","scope":"project","paths":["src/**"]}"#
        )
        .item_id
            == "web",
        "non-file http(s) URLs must still persist"
    );
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"see file://user:secret@localhost/etc/passwd"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"see https://user:pass@example.com/docs"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"paths":["%2FUsers%2Fsecret"]}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"%2FUsers%2Fsecret"}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"paths":["%2e%2e/%2e%2e/etc/passwd"]}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"paths":["%5c%5cserver%5cshare"]}"#,
    ));
    assert_privacy(MetadataItem::new(
        "a",
        "ok",
        "desc",
        r#"{"when_to_use":"see file:%2f%2f%2fetc"}"#,
    ));

    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    let bypass = MetadataItem {
        item_id: "a".into(),
        content_hash: "deadbeef".into(),
        name: "ok".into(),
        description: "desc".into(),
        extra: r#"{"when_to_use":"/Users/foo/.ssh/id_rsa"}"#.into(),
    };
    let err = idx
        .replace_inventory(CollectionKind::Skills, 1, &[bypass])
        .unwrap_err();
    assert!(matches!(err, MetadataIndexError::Privacy(_)));
    let unix = MetadataItem {
        item_id: "b".into(),
        content_hash: "deadbeef".into(),
        name: "ok".into(),
        description: "see /etc/passwd".into(),
        extra: "".into(),
    };
    let err = idx
        .replace_inventory(CollectionKind::Skills, 1, &[unix])
        .unwrap_err();
    assert!(matches!(err, MetadataIndexError::Privacy(_)));
    let windows = MetadataItem {
        item_id: "c".into(),
        content_hash: "deadbeef".into(),
        name: "ok".into(),
        description: r"see C:\Windows\system32".into(),
        extra: "".into(),
    };
    let err = idx
        .replace_inventory(CollectionKind::Skills, 1, &[windows])
        .unwrap_err();
    assert!(matches!(err, MetadataIndexError::Privacy(_)));
    let file_url = MetadataItem {
        item_id: "d".into(),
        content_hash: "deadbeef".into(),
        name: "ok".into(),
        description: "desc".into(),
        extra: r#"{"when_to_use":"see file:///etc/passwd"}"#.into(),
    };
    let err = idx
        .replace_inventory(CollectionKind::Skills, 1, &[file_url])
        .unwrap_err();
    assert!(matches!(err, MetadataIndexError::Privacy(_)));
    let localhost_url = MetadataItem {
        item_id: "e".into(),
        content_hash: "deadbeef".into(),
        name: "ok".into(),
        description: "desc".into(),
        extra: r#"{"when_to_use":"see file://localhost/etc/passwd"}"#.into(),
    };
    let err = idx
        .replace_inventory(CollectionKind::Skills, 1, &[localhost_url])
        .unwrap_err();
    assert!(matches!(err, MetadataIndexError::Privacy(_)));
    let file_single = MetadataItem {
        item_id: "e2".into(),
        content_hash: "deadbeef".into(),
        name: "ok".into(),
        description: "see file:/etc/passwd".into(),
        extra: "".into(),
    };
    let err = idx
        .replace_inventory(CollectionKind::Skills, 1, &[file_single])
        .unwrap_err();
    assert!(matches!(err, MetadataIndexError::Privacy(_)));
    let encoded_file = MetadataItem {
        item_id: "f".into(),
        content_hash: "deadbeef".into(),
        name: "ok".into(),
        description: "desc".into(),
        extra: r#"{"when_to_use":"see file:%2f%2f%2fetc/passwd"}"#.into(),
    };
    let err = idx
        .replace_inventory(CollectionKind::Skills, 1, &[encoded_file])
        .unwrap_err();
    assert!(matches!(err, MetadataIndexError::Privacy(_)));
    let encoded_localhost = MetadataItem {
        item_id: "g".into(),
        content_hash: "deadbeef".into(),
        name: "ok".into(),
        description: "desc".into(),
        extra: r#"{"when_to_use":"see file:%2flocalhost%2fetc%2fpasswd"}"#.into(),
    };
    let err = idx
        .replace_inventory(CollectionKind::Skills, 1, &[encoded_localhost])
        .unwrap_err();
    assert!(matches!(err, MetadataIndexError::Privacy(_)));
    let traversal = MetadataItem {
        item_id: "h".into(),
        content_hash: "deadbeef".into(),
        name: "ok".into(),
        description: "see file:./../../etc/passwd".into(),
        extra: "".into(),
    };
    let err = idx
        .replace_inventory(CollectionKind::Skills, 1, &[traversal])
        .unwrap_err();
    assert!(matches!(err, MetadataIndexError::Privacy(_)));
    let parent = MetadataItem {
        item_id: "i".into(),
        content_hash: "deadbeef".into(),
        name: "ok".into(),
        description: "desc".into(),
        extra: r#"{"when_to_use":"file:../etc/passwd"}"#.into(),
    };
    let err = idx
        .replace_inventory(CollectionKind::Skills, 1, &[parent])
        .unwrap_err();
    assert!(matches!(err, MetadataIndexError::Privacy(_)));
    let backslash_file = MetadataItem {
        item_id: "j".into(),
        content_hash: "deadbeef".into(),
        name: "ok".into(),
        description: r"see file:\///etc/passwd".into(),
        extra: "".into(),
    };
    let err = idx
        .replace_inventory(CollectionKind::Skills, 1, &[backslash_file])
        .unwrap_err();
    assert!(matches!(err, MetadataIndexError::Privacy(_)));
    let allowed = extra_item(
        "ok",
        "lint",
        "lints rust",
        r#"{"when_to_use":"see file:./notes.md https://example.com/docs","scope":"project","paths":["src/**"]}"#,
    );
    idx.replace_inventory(CollectionKind::Skills, 2, &[allowed])
        .unwrap();
    let cells = idx.text_cells().unwrap().join("\n");
    for forbidden in [
        "/Users/",
        "/home/",
        "~/",
        "\\Users\\",
        "\\\\nas",
        "/etc/passwd",
        r"C:\Windows\system32",
        "/usr/bin/git",
        "file:///etc/passwd",
        "file://localhost/etc/passwd",
        "file:/etc/passwd",
        "FILE:/etc/passwd",
        "file:%2f%2f%2fetc/passwd",
        "file:%2flocalhost%2fetc%2fpasswd",
        "file:./../../etc/passwd",
        "file:../etc/passwd",
        r"file:\///etc/passwd",
    ] {
        assert!(
            !cells.contains(forbidden),
            "metadata db leaked {forbidden:?}"
        );
    }
    assert!(
        cells.contains("file:./notes.md"),
        "relative file:./ must persist into extra/fts"
    );
    assert!(
        cells.contains("https://example.com/docs"),
        "non-file http(s) URLs must persist into extra/fts"
    );
}

#[test]
fn database_contains_no_forbidden_privacy_material() {
    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    let extra = r#"{"when_to_use":"when formatting rust","scope":"project","paths":["src/**"]}"#;
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[extra_item(
            "fmt",
            "rust-format",
            "formats rust sources",
            extra,
        )],
    )
    .unwrap();
    idx.replace_inventory(
        CollectionKind::CallableAgents,
        1,
        &[item("explore", "explore", "read-only codebase explorer")],
    )
    .unwrap();

    let cells = idx.text_cells().unwrap().join("\n").to_ascii_lowercase();
    for forbidden in [
        "sk-",
        "api_key",
        "authorization",
        "bearer ",
        "/users/",
        "/home/",
        "session history",
        "-----begin",
        "raw provider error",
        "0.39215687",
        "you are a helpful assistant",
        "fn main() {",
    ] {
        assert!(
            !cells.contains(forbidden),
            "metadata db leaked {forbidden:?}"
        );
    }
    assert!(cells.contains("rust-format"));
    assert!(cells.contains("explore"));
}

#[test]
fn incremental_upsert_skips_unchanged_and_deletes_missing() {
    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    let a = item("fmt", "rust-format", "formats rust");
    let b = item("lint", "rust-lint", "lints rust");
    let r1 = idx
        .replace_inventory(CollectionKind::Skills, 1, &[a.clone(), b.clone()])
        .unwrap();
    assert_eq!(r1.added, 2);
    let r2 = idx
        .replace_inventory(CollectionKind::Skills, 2, &[a.clone(), b.clone()])
        .unwrap();
    assert_eq!(r2.unchanged, 2);
    assert_eq!(r2.added, 0);
    let a2 = item("fmt", "rust-format", "formats rust files");
    let r3 = idx
        .replace_inventory(CollectionKind::Skills, 3, &[a2])
        .unwrap();
    assert_eq!(r3.updated, 1);
    assert_eq!(r3.removed, 1);
    assert_eq!(idx.item_count(CollectionKind::Skills), 1);
    assert_eq!(idx.item_count(CollectionKind::CallableAgents), 0);
}

#[test]
fn fts_search_finds_indexed_metadata() {
    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust sources")],
    )
    .unwrap();
    idx.replace_inventory(
        CollectionKind::CallableAgents,
        1,
        &[item("explore", "explore", "codebase explorer agent")],
    )
    .unwrap();
    let hits = idx
        .search_fts(CollectionKind::Skills, "rust format", 8)
        .unwrap();
    assert!(hits.iter().any(|h| h.item_id == "fmt"));
    let agent_hits = idx
        .search_fts(CollectionKind::CallableAgents, "explorer", 8)
        .unwrap();
    assert!(agent_hits.iter().any(|h| h.item_id == "explore"));
    let cross = idx
        .search_fts(CollectionKind::Skills, "explorer", 8)
        .unwrap();
    assert!(cross.is_empty());
}

#[tokio::test]
async fn fts_only_degradation_works_without_sqlite_vec() {
    let tmp = TempDir::new().unwrap();
    init_sqlite_vec();
    let db_path = tmp.path().join("meta.sqlite");
    super::set_force_fts_only(true);
    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    assert!(!idx.vec_available());
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust sources")],
    )
    .unwrap();
    let hits = idx
        .search_fts(CollectionKind::Skills, "rust format", 8)
        .unwrap();
    assert!(!hits.is_empty());
    let knn = idx
        .search_knn(CollectionKind::Skills, &[0.1, 0.2, 0.3, 0.4], 4)
        .unwrap();
    assert!(knn.is_empty());
    drop(idx);
    let out = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        None,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    super::set_force_fts_only(false);
    assert!(matches!(out, VectorReadiness::Disabled), "{out:?}");
}

#[tokio::test]
async fn skills_rebuild_never_drops_or_blocks_callable_agents() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust sources")],
    )
    .unwrap();
    idx.replace_inventory(
        CollectionKind::CallableAgents,
        1,
        &[item("explore", "explore", "codebase explorer agent")],
    )
    .unwrap();
    drop(idx);

    let embedder: Option<Arc<dyn EmbeddingProvider>> =
        Some(Arc::new(MockEmbeddingProvider { dimensions: 4 }));
    let skills_ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder.clone(),
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(skills_ready, VectorReadiness::Ready),
        "{skills_ready:?}"
    );
    let agents_ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::CallableAgents,
        &spec("m", 4),
        embedder.clone(),
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(agents_ready, VectorReadiness::Ready),
        "{agents_ready:?}"
    );

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    assert_eq!(idx.vec_count(CollectionKind::Skills), 1);
    assert_eq!(idx.vec_count(CollectionKind::CallableAgents), 1);
    let agent_fp = idx
        .collection_state(CollectionKind::CallableAgents)
        .unwrap()
        .fingerprint_hash
        .clone();
    drop(idx);

    let rebuilt = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m-other", 4),
        embedder,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(rebuilt, VectorReadiness::Ready), "{rebuilt:?}");

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    assert_eq!(idx.item_count(CollectionKind::CallableAgents), 1);
    assert_eq!(idx.vec_count(CollectionKind::CallableAgents), 1);
    assert_eq!(
        idx.collection_state(CollectionKind::CallableAgents)
            .unwrap()
            .fingerprint_hash,
        agent_fp
    );
    assert_eq!(idx.vec_count(CollectionKind::Skills), 1);
}

#[test]
fn contention_claims_are_collection_local() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx_a) = open(&tmp);
    let idx_b = MetadataIndex::open_or_create(&db_path).unwrap();
    let mut pending_skills =
        ensure_pending(&idx_a, CollectionKind::Skills, "fp-skills", "test").unwrap();
    let mut pending_agents =
        ensure_pending(&idx_b, CollectionKind::CallableAgents, "fp-agents", "test").unwrap();
    assert!(try_claim_rebuild(
        &idx_a,
        CollectionKind::Skills,
        &mut pending_skills,
        60
    ));
    assert!(
        try_claim_rebuild(
            &idx_b,
            CollectionKind::CallableAgents,
            &mut pending_agents,
            60
        ),
        "callable_agents claim must not be blocked by a skills claim"
    );
    // Same-process reclaim is allowed; simulate a live foreign owner so
    // same-collection CAS still serializes across processes.
    let mut foreign = pending_state_required(&idx_b, CollectionKind::Skills);
    foreign.claim = "999999:9999999999".into();
    foreign.claimed_at = 9_999_999_999;
    foreign.status = "running".into();
    idx_b
        .db()
        .execute(
            "UPDATE collections SET pending_json = ?1 WHERE name = ?2",
            rusqlite::params![foreign.to_json(), CollectionKind::Skills.as_str()],
        )
        .unwrap();
    let mut pending_skills_b = pending_state_required(&idx_b, CollectionKind::Skills);
    assert!(
        !try_claim_rebuild(&idx_b, CollectionKind::Skills, &mut pending_skills_b, 3600),
        "same-collection claim must serialize against a live foreign owner"
    );
    assert_eq!(
        super::rebuild::pending_state(&idx_b, CollectionKind::CallableAgents)
            .unwrap()
            .status,
        "running",
        "foreign skills owner must not drop the callable_agents claim"
    );
}

fn pending_state_required(
    idx: &MetadataIndex,
    collection: CollectionKind,
) -> super::rebuild::CollectionPending {
    super::rebuild::pending_state(idx, collection).expect("pending")
}

#[test]
fn crash_claim_pre_commit_rolls_back() {
    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    let mut pending = ensure_pending(&idx, CollectionKind::Skills, "fp", "test").unwrap();
    CLAIM_PRE_COMMIT_FAIL.store(true, std::sync::atomic::Ordering::SeqCst);
    let won = try_claim_rebuild(&idx, CollectionKind::Skills, &mut pending, 60);
    CLAIM_PRE_COMMIT_FAIL.store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(!won);
    let stored = super::rebuild::pending_state(&idx, CollectionKind::Skills).unwrap();
    assert_ne!(stored.status, "running");
    assert!(stored.claim.is_empty());
}

#[test]
fn stale_hashes_cannot_install_vectors() {
    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    let live = item("fmt", "rust-format", "formats rust");
    idx.replace_inventory(CollectionKind::Skills, 1, &[live.clone()])
        .unwrap();
    let source = spec("m", 4);
    let (fp, payload) = fp_for(CollectionKind::Skills, &source);
    let pending = ensure_pending(&idx, CollectionKind::Skills, &fp.hash, "test").unwrap();
    let stale_hash = "deadbeefdeadbeefdeadbeefdeadbeef";
    stage_vector(
        &idx,
        CollectionKind::Skills,
        &pending.id,
        &fp.hash,
        "fmt",
        stale_hash,
        &[0.1, 0.2, 0.3, 0.4],
    )
    .unwrap();
    assert!(!staging_complete(&idx, CollectionKind::Skills, &pending.id).unwrap());
    let installed =
        install_vectors(&idx, CollectionKind::Skills, &pending, &fp, &payload, 4).unwrap();
    assert!(!installed);
    assert_eq!(idx.vec_count(CollectionKind::Skills), 0);
    assert!(
        idx.collection_state(CollectionKind::Skills)
            .unwrap()
            .fingerprint_hash
            .is_empty()
    );
}

#[test]
fn crash_install_pre_commit_keeps_old_vectors() {
    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    let live = item("fmt", "rust-format", "formats rust");
    idx.replace_inventory(CollectionKind::Skills, 1, &[live.clone()])
        .unwrap();
    idx.replace_inventory(
        CollectionKind::CallableAgents,
        1,
        &[item("explore", "explore", "explorer")],
    )
    .unwrap();
    let source = spec("m", 4);
    let (fp_agents, payload_agents) = fp_for(CollectionKind::CallableAgents, &source);
    let pending_agents = ensure_pending(
        &idx,
        CollectionKind::CallableAgents,
        &fp_agents.hash,
        "test",
    )
    .unwrap();
    stage_vector(
        &idx,
        CollectionKind::CallableAgents,
        &pending_agents.id,
        &fp_agents.hash,
        "explore",
        &item("explore", "explore", "explorer").content_hash,
        &[0.4, 0.3, 0.2, 0.1],
    )
    .unwrap();
    assert!(
        install_vectors(
            &idx,
            CollectionKind::CallableAgents,
            &pending_agents,
            &fp_agents,
            &payload_agents,
            4,
        )
        .unwrap()
    );
    assert_eq!(idx.vec_count(CollectionKind::CallableAgents), 1);

    let (fp, payload) = fp_for(CollectionKind::Skills, &source);
    let pending = ensure_pending(&idx, CollectionKind::Skills, &fp.hash, "test").unwrap();
    stage_vector(
        &idx,
        CollectionKind::Skills,
        &pending.id,
        &fp.hash,
        "fmt",
        &live.content_hash,
        &[0.1, 0.2, 0.3, 0.4],
    )
    .unwrap();
    INSTALL_PRE_COMMIT_FAIL.store(true, std::sync::atomic::Ordering::SeqCst);
    let installed =
        install_vectors(&idx, CollectionKind::Skills, &pending, &fp, &payload, 4).unwrap();
    INSTALL_PRE_COMMIT_FAIL.store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(!installed);
    assert!(
        idx.collection_state(CollectionKind::Skills)
            .unwrap()
            .fingerprint_hash
            .is_empty()
    );
    assert_eq!(idx.vec_count(CollectionKind::CallableAgents), 1);
}

#[test]
fn crash_install_pre_commit_keeps_same_collection_vectors() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    let live = item("fmt", "rust-format", "formats rust");
    idx.replace_inventory(CollectionKind::Skills, 1, &[live.clone()])
        .unwrap();
    let source_a = spec("m-a", 4);
    let (fp_a, payload_a) = fp_for(CollectionKind::Skills, &source_a);
    let pending_a = ensure_pending(&idx, CollectionKind::Skills, &fp_a.hash, "test").unwrap();
    stage_vector(
        &idx,
        CollectionKind::Skills,
        &pending_a.id,
        &fp_a.hash,
        "fmt",
        &live.content_hash,
        &[0.1, 0.2, 0.3, 0.4],
    )
    .unwrap();
    assert!(
        install_vectors(
            &idx,
            CollectionKind::Skills,
            &pending_a,
            &fp_a,
            &payload_a,
            4,
        )
        .unwrap()
    );
    let hash_a = idx
        .collection_state(CollectionKind::Skills)
        .unwrap()
        .fingerprint_hash;
    assert_eq!(hash_a, fp_a.hash);
    assert_eq!(idx.vec_count(CollectionKind::Skills), 1);
    drop(idx);

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let source_b = spec("m-b", 4);
    let (fp_b, payload_b) = fp_for(CollectionKind::Skills, &source_b);
    let pending_b = ensure_pending(&idx, CollectionKind::Skills, &fp_b.hash, "test").unwrap();
    stage_vector(
        &idx,
        CollectionKind::Skills,
        &pending_b.id,
        &fp_b.hash,
        "fmt",
        &live.content_hash,
        &[0.4, 0.3, 0.2, 0.1],
    )
    .unwrap();
    INSTALL_PRE_COMMIT_FAIL.store(true, std::sync::atomic::Ordering::SeqCst);
    let installed = install_vectors(
        &idx,
        CollectionKind::Skills,
        &pending_b,
        &fp_b,
        &payload_b,
        4,
    )
    .unwrap();
    INSTALL_PRE_COMMIT_FAIL.store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(!installed);
    drop(idx);

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let state = idx.collection_state(CollectionKind::Skills).unwrap();
    assert_eq!(state.fingerprint_hash, hash_a);
    assert_eq!(idx.vec_count(CollectionKind::Skills), 1);
    let knn = idx
        .search_knn(CollectionKind::Skills, &[0.1, 0.2, 0.3, 0.4], 4)
        .unwrap();
    assert_eq!(knn.first().map(|h| h.item_id.as_str()), Some("fmt"));
}

#[test]
fn none_vs_l2_v1_fingerprint_mismatch_forces_rebuild_without_mixing() {
    let none = spec("m", 4);
    let mut l2 = none.clone();
    l2.normalization = NORMALIZATION_L2_V1.into();
    let (fp_none, _) = fp_for(CollectionKind::Skills, &none);
    let (fp_l2, _) = fp_for(CollectionKind::Skills, &l2);
    assert_ne!(
        fp_none.hash, fp_l2.hash,
        "normalization label is a vector-space identity"
    );
}

#[test]
fn discard_collection_rebuild_crash_retains_pending_and_staging() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    let pending = ensure_pending(&idx, CollectionKind::Skills, "fp", "test").unwrap();
    stage_vector(
        &idx,
        CollectionKind::Skills,
        &pending.id,
        "fp",
        "fmt",
        &item("fmt", "rust-format", "formats rust").content_hash,
        &[0.1, 0.2, 0.3, 0.4],
    )
    .unwrap();
    drop(idx);

    DISCARD_PRE_COMMIT_FAIL.store(true, std::sync::atomic::Ordering::SeqCst);
    discard_collection_rebuild(&db_path, CollectionKind::Skills);
    DISCARD_PRE_COMMIT_FAIL.store(false, std::sync::atomic::Ordering::SeqCst);

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    assert!(
        pending_marker_present(&idx, CollectionKind::Skills),
        "crash before commit must retain pending_json"
    );
    assert!(
        staged_count(&idx, CollectionKind::Skills, &pending.id) > 0,
        "crash before commit must retain staging"
    );
}

#[test]
fn upsert_embedding_rejects_nan_and_wrong_dim() {
    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    let live = item("fmt", "rust-format", "formats rust");
    idx.replace_inventory(CollectionKind::Skills, 1, &[live.clone()])
        .unwrap();
    let source = spec("m", 4);
    let (fp, payload) = fp_for(CollectionKind::Skills, &source);
    let pending = ensure_pending(&idx, CollectionKind::Skills, &fp.hash, "test").unwrap();
    stage_vector(
        &idx,
        CollectionKind::Skills,
        &pending.id,
        &fp.hash,
        "fmt",
        &live.content_hash,
        &[0.1, 0.2, 0.3, 0.4],
    )
    .unwrap();
    assert!(install_vectors(&idx, CollectionKind::Skills, &pending, &fp, &payload, 4).unwrap());
    let count = idx.vec_count(CollectionKind::Skills);
    let knn_before = idx
        .search_knn(CollectionKind::Skills, &[0.1, 0.2, 0.3, 0.4], 4)
        .unwrap();
    let nan_err = idx
        .upsert_embedding(CollectionKind::Skills, "fmt", &[f32::NAN, 1.0, 2.0, 3.0])
        .unwrap_err();
    assert!(matches!(nan_err, MetadataIndexError::InvalidItem(_)));
    let dim_err = idx
        .upsert_embedding(CollectionKind::Skills, "fmt", &[0.1, 0.2])
        .unwrap_err();
    assert!(matches!(dim_err, MetadataIndexError::InvalidItem(_)));
    assert_eq!(idx.vec_count(CollectionKind::Skills), count);
    let knn_after = idx
        .search_knn(CollectionKind::Skills, &[0.1, 0.2, 0.3, 0.4], 4)
        .unwrap();
    assert_eq!(knn_before, knn_after);
}

#[test]
fn items_without_embeddings_surfaces_ids_when_rowids_shadow_is_missing() {
    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    let live = item("fmt", "rust-format", "formats rust");
    idx.replace_inventory(CollectionKind::Skills, 1, &[live.clone()])
        .unwrap();
    let source = spec("m", 4);
    let (fp, payload) = fp_for(CollectionKind::Skills, &source);
    let pending = ensure_pending(&idx, CollectionKind::Skills, &fp.hash, "test").unwrap();
    stage_vector(
        &idx,
        CollectionKind::Skills,
        &pending.id,
        &fp.hash,
        "fmt",
        &live.content_hash,
        &[0.1, 0.2, 0.3, 0.4],
    )
    .unwrap();
    assert!(install_vectors(&idx, CollectionKind::Skills, &pending, &fp, &payload, 4).unwrap());
    let _ = idx
        .db()
        .execute("DROP TABLE IF EXISTS skills_vec_rowids", []);
    let missing = idx
        .items_without_embeddings(CollectionKind::Skills)
        .unwrap();
    assert!(
        missing.iter().any(|(id, _)| id == "fmt"),
        "torn vec0 shadow must still surface live ids: {missing:?}"
    );
}

#[test]
fn network_filesystem_uses_truncate_per_host_db() {
    if std::env::var("GROK_SQLITE_JOURNAL_MODE").is_ok() {
        return;
    }
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("metadata.sqlite");
    let idx =
        MetadataIndex::open_with_journal_mode_for_test(&db_path, JournalMode::Truncate).unwrap();
    assert_eq!(journal_mode(&idx), "truncate");
    drop(idx);
    let eff = JournalMode::Truncate.effective_db_path(&db_path);
    assert_ne!(eff, db_path);
    assert!(eff.exists());
    let base = eff.display().to_string();
    assert!(!std::fs::exists(format!("{base}-wal")).unwrap());
    assert!(!std::fs::exists(format!("{base}-shm")).unwrap());
}

#[test]
fn path_uses_shared_workspace_identity() {
    let home = std::path::Path::new("/tmp/grok-test-home");
    let cwd = std::path::Path::new("/users/me/work/demo-project");
    let identity = workspace_storage_identity(cwd);
    let path = metadata_index_path_for_cwd(home, cwd);
    assert_eq!(path, metadata_index_path(home, &identity));
    assert!(path.ends_with("metadata.sqlite"));
    let rendered = path.to_string_lossy();
    assert!(rendered.contains("indexes/prime/"));
    assert!(rendered.contains(&identity));
    assert!(!identity.starts_with('/'));
}

#[tokio::test]
async fn stage_only_does_not_install_until_commit() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    drop(idx);
    let embedder: Option<Arc<dyn EmbeddingProvider>> =
        Some(Arc::new(MockEmbeddingProvider { dimensions: 4 }));
    let staged = stage_collection_vectors(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(staged, VectorReadiness::Pending { owned: true }),
        "stage-only must not report Ready, got {staged:?}"
    );
    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let state = idx.collection_state(CollectionKind::Skills).unwrap();
    assert!(
        state.fingerprint_hash.is_empty(),
        "stage-only must not commit a live fingerprint"
    );
    assert_eq!(state.vec_count, 0);
    drop(idx);
    assert!(
        commit_staged_vectors(&db_path, CollectionKind::Skills, &spec("m", 4)),
        "complete staging must install after an explicit commit"
    );
    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let state = idx.collection_state(CollectionKind::Skills).unwrap();
    assert!(!state.fingerprint_hash.is_empty());
    assert!(state.vec_count > 0);
}

#[tokio::test]
async fn incremental_compatible_gap_is_ready_missing_not_rebuild() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    drop(idx);
    let embedder: Option<Arc<dyn EmbeddingProvider>> =
        Some(Arc::new(MockEmbeddingProvider { dimensions: 4 }));
    let ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder.clone(),
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(ready, VectorReadiness::Ready), "{ready:?}");

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    idx.replace_inventory(
        CollectionKind::Skills,
        2,
        &[
            item("fmt", "rust-format", "formats rust"),
            item("lint", "rust-lint", "lints rust"),
        ],
    )
    .unwrap();
    drop(idx);

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let state = idx.collection_state(CollectionKind::Skills).unwrap();
    assert!(!state.fingerprint_hash.is_empty());
    assert_eq!(idx.item_count(CollectionKind::Skills), 2);
    // Changed inventory leaves the previous vector row; the extra item is a
    // compatible gap, not a reason to drop the collection.
    assert_eq!(idx.vec_count(CollectionKind::Skills), 1);
    assert!(idx.vectors_safe_to_backfill(CollectionKind::Skills));
    let missing = idx
        .items_without_embeddings(CollectionKind::Skills)
        .unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].0, "lint");
    drop(idx);

    let gap = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder.clone(),
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(gap, VectorReadiness::ReadyMissing { missing: 1 }),
        "{gap:?}"
    );

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let missing = idx
        .items_without_embeddings(CollectionKind::Skills)
        .unwrap();
    assert_eq!(missing.len(), 1);
    let (new_id, text) = missing.into_iter().next().unwrap();
    let mock = MockEmbeddingProvider { dimensions: 4 };
    let vectors = mock.embed_batch(&[text.as_str()]).await.unwrap();
    idx.upsert_embedding(CollectionKind::Skills, &new_id, &vectors[0])
        .unwrap();
    assert_eq!(idx.vec_count(CollectionKind::Skills), 2);
    let knn = idx
        .search_knn(CollectionKind::Skills, &vectors[0], 4)
        .unwrap();
    assert!(
        knn.iter().any(|h| h.item_id == "lint"),
        "backfilled id must be reachable by KNN: {knn:?}"
    );
    drop(idx);

    let ready2 = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder.clone(),
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(ready2, VectorReadiness::Ready), "{ready2:?}");

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    idx.replace_inventory(
        CollectionKind::Skills,
        3,
        &[
            item("fmt", "rust-format", "formats rust files"),
            item("lint", "rust-lint", "lints rust"),
        ],
    )
    .unwrap();
    assert_eq!(idx.vec_count(CollectionKind::Skills), 1);
    let missing = idx
        .items_without_embeddings(CollectionKind::Skills)
        .unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].0, "fmt");
    drop(idx);

    let gap_update = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(gap_update, VectorReadiness::ReadyMissing { missing: 1 }),
        "{gap_update:?}"
    );
    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let missing = idx
        .items_without_embeddings(CollectionKind::Skills)
        .unwrap();
    let (updated_id, text) = missing.into_iter().next().unwrap();
    let mock = MockEmbeddingProvider { dimensions: 4 };
    let vectors = mock.embed_batch(&[text.as_str()]).await.unwrap();
    idx.upsert_embedding(CollectionKind::Skills, &updated_id, &vectors[0])
        .unwrap();
    let knn = idx
        .search_knn(CollectionKind::Skills, &vectors[0], 4)
        .unwrap();
    assert!(
        knn.iter().any(|h| h.item_id == "fmt"),
        "updated id must be reachable by KNN after backfill: {knn:?}"
    );
}

fn embedding_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|f| f.to_le_bytes()).collect()
}

#[tokio::test]
async fn compatible_readiness_prunes_orphan_vector_rows() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    idx.replace_inventory(
        CollectionKind::CallableAgents,
        1,
        &[item("explore", "explore", "explorer")],
    )
    .unwrap();
    drop(idx);
    let embedder: Option<Arc<dyn EmbeddingProvider>> =
        Some(Arc::new(MockEmbeddingProvider { dimensions: 4 }));
    let ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder.clone(),
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(ready, VectorReadiness::Ready), "{ready:?}");
    let agents_ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::CallableAgents,
        &spec("m", 4),
        embedder.clone(),
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(agents_ready, VectorReadiness::Ready),
        "{agents_ready:?}"
    );

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let agent_fp = idx
        .collection_state(CollectionKind::CallableAgents)
        .unwrap()
        .fingerprint_hash
        .clone();
    let query = [0.1f32, 0.2, 0.3, 0.4];
    idx.db()
        .execute(
            "INSERT OR REPLACE INTO skills_vec(item_id, embedding) VALUES (?1, ?2)",
            rusqlite::params!["ghost", embedding_bytes(&query)],
        )
        .unwrap();
    assert_eq!(idx.item_count(CollectionKind::Skills), 1);
    assert_eq!(idx.vec_count(CollectionKind::Skills), 2);
    let knn = idx.search_knn(CollectionKind::Skills, &query, 4).unwrap();
    assert!(
        knn.iter().all(|h| h.item_id != "ghost"),
        "ghost vec row must never be KNN-queryable, even before prune: {knn:?}"
    );
    assert!(
        knn.iter().any(|h| h.item_id == "fmt"),
        "live items must remain KNN-queryable beside a ghost: {knn:?}"
    );
    drop(idx);

    let out = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(out, VectorReadiness::Ready),
        "orphan prune must yield Ready, not a rebuild: {out:?}"
    );

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    assert_eq!(idx.item_count(CollectionKind::Skills), 1);
    assert_eq!(idx.vec_count(CollectionKind::Skills), 1);
    let knn = idx.search_knn(CollectionKind::Skills, &query, 4).unwrap();
    assert!(
        knn.iter().all(|h| h.item_id != "ghost"),
        "pruned ghost must not be KNN-queryable: {knn:?}"
    );
    assert_eq!(idx.vec_count(CollectionKind::CallableAgents), 1);
    assert_eq!(
        idx.collection_state(CollectionKind::CallableAgents)
            .unwrap()
            .fingerprint_hash,
        agent_fp,
        "skills orphan prune must not touch callable_agents"
    );
    assert_eq!(idx.prune_orphan_vector_rows(CollectionKind::Skills), 0);
}

#[tokio::test]
async fn upsert_embedding_rejects_ghost_and_unsafe_backfill() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    drop(idx);
    let embedder: Option<Arc<dyn EmbeddingProvider>> =
        Some(Arc::new(MockEmbeddingProvider { dimensions: 4 }));
    let ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(ready, VectorReadiness::Ready), "{ready:?}");

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let vector = [0.1f32, 0.2, 0.3, 0.4];
    let err = idx
        .upsert_embedding(CollectionKind::Skills, "ghost", &vector)
        .unwrap_err();
    assert!(
        matches!(err, MetadataIndexError::InvalidItem(_)),
        "ghost upsert must not install a KNN-queryable row: {err:?}"
    );
    assert_eq!(idx.vec_count(CollectionKind::Skills), 1);
    let knn = idx.search_knn(CollectionKind::Skills, &vector, 4).unwrap();
    assert!(
        knn.iter().all(|h| h.item_id != "ghost"),
        "rejected ghost must stay absent from KNN: {knn:?}"
    );

    idx.replace_inventory(
        CollectionKind::Skills,
        2,
        &[
            item("fmt", "rust-format", "formats rust"),
            item("lint", "rust-lint", "lints rust"),
        ],
    )
    .unwrap();
    assert_eq!(idx.vec_count(CollectionKind::Skills), 1);
    let pending = ensure_pending(&idx, CollectionKind::Skills, "fp-pending", "test").unwrap();
    assert!(!idx.vectors_safe_to_backfill(CollectionKind::Skills));
    idx.upsert_embedding(CollectionKind::Skills, "lint", &vector)
        .unwrap();
    assert_eq!(
        idx.vec_count(CollectionKind::Skills),
        1,
        "unsafe backfill must not write the live vec table"
    );
    let missing = idx
        .items_without_embeddings(CollectionKind::Skills)
        .unwrap();
    assert!(
        missing.iter().any(|(id, _)| id == "lint"),
        "gap item must remain unembedded while a rebuild is pending"
    );
    assert_eq!(
        staged_count(&idx, CollectionKind::Skills, &pending.id),
        0,
        "live upsert must not write staging"
    );
}

#[test]
fn sibling_collection_helper_is_symmetric() {
    assert_eq!(
        super::rebuild::sibling(CollectionKind::Skills),
        CollectionKind::CallableAgents
    );
    assert_eq!(
        super::rebuild::sibling(CollectionKind::CallableAgents),
        CollectionKind::Skills
    );
}

#[tokio::test]
async fn leftover_pending_on_compatible_collection_is_cleared() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    idx.replace_inventory(
        CollectionKind::CallableAgents,
        1,
        &[item("explore", "explore", "explorer")],
    )
    .unwrap();
    drop(idx);
    let embedder: Option<Arc<dyn EmbeddingProvider>> =
        Some(Arc::new(MockEmbeddingProvider { dimensions: 4 }));
    let ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder.clone(),
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(ready, VectorReadiness::Ready), "{ready:?}");

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let fp_hash = idx
        .collection_state(CollectionKind::Skills)
        .unwrap()
        .fingerprint_hash
        .clone();
    let leftover = ensure_pending(&idx, CollectionKind::Skills, &fp_hash, "stale").unwrap();
    stage_vector(
        &idx,
        CollectionKind::Skills,
        &leftover.id,
        &fp_hash,
        "fmt",
        &item("fmt", "rust-format", "formats rust").content_hash,
        &[0.1, 0.2, 0.3, 0.4],
    )
    .unwrap();
    let agents_pending =
        ensure_pending(&idx, CollectionKind::CallableAgents, "fp-agents", "test").unwrap();
    assert!(pending_marker_present(&idx, CollectionKind::Skills));
    drop(idx);

    let out = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    assert!(
        !pending_marker_present(&idx, CollectionKind::Skills),
        "compatible leftover pending_json must be empty"
    );
    assert_eq!(staged_count(&idx, CollectionKind::Skills, &leftover.id), 0);
    assert_eq!(
        super::rebuild::pending_state(&idx, CollectionKind::CallableAgents)
            .unwrap()
            .id,
        agents_pending.id,
        "clearing skills must not drop a sibling pending marker"
    );
}

#[test]
fn clear_completed_target_crash_does_not_tear_pending_and_staging() {
    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    let pending = ensure_pending(&idx, CollectionKind::Skills, "fp", "test").unwrap();
    stage_vector(
        &idx,
        CollectionKind::Skills,
        &pending.id,
        "fp",
        "fmt",
        &item("fmt", "rust-format", "formats rust").content_hash,
        &[0.1, 0.2, 0.3, 0.4],
    )
    .unwrap();

    CLEAR_PRE_COMMIT_FAIL.store(true, std::sync::atomic::Ordering::SeqCst);
    clear_completed_target(&idx, CollectionKind::Skills, Some(&pending.id), "fp");
    CLEAR_PRE_COMMIT_FAIL.store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(
        pending_marker_present(&idx, CollectionKind::Skills),
        "crash before commit must retain pending_json"
    );
    assert!(
        staged_count(&idx, CollectionKind::Skills, &pending.id) > 0,
        "crash before commit must retain staging"
    );

    clear_completed_target(&idx, CollectionKind::Skills, Some(&pending.id), "fp");
    assert!(!pending_marker_present(&idx, CollectionKind::Skills));
    assert_eq!(staged_count(&idx, CollectionKind::Skills, &pending.id), 0);
}

#[tokio::test]
async fn concurrent_rebuild_loser_observes_ready_without_pending() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    drop(idx);
    let embedder_a: Option<Arc<dyn EmbeddingProvider>> =
        Some(Arc::new(MockEmbeddingProvider { dimensions: 4 }));
    let embedder_b = embedder_a.clone();
    let path_a = db_path.clone();
    let path_b = db_path.clone();
    let spec_a = spec("m", 4);
    let spec_b = spec("m", 4);
    let (first, second) = tokio::join!(
        ensure_collection_vectors_ready(
            &path_a,
            CollectionKind::Skills,
            &spec_a,
            embedder_a,
            60,
            0,
            Some(8),
            CancellationToken::new(),
        ),
        ensure_collection_vectors_ready(
            &path_b,
            CollectionKind::Skills,
            &spec_b,
            embedder_b.clone(),
            60,
            0,
            Some(8),
            CancellationToken::new(),
        ),
    );
    let first = if matches!(first, VectorReadiness::Pending { .. }) {
        ensure_collection_vectors_ready(
            &db_path,
            CollectionKind::Skills,
            &spec("m", 4),
            embedder_b.clone(),
            60,
            0,
            Some(8),
            CancellationToken::new(),
        )
        .await
    } else {
        first
    };
    let second = if matches!(second, VectorReadiness::Pending { .. }) {
        ensure_collection_vectors_ready(
            &db_path,
            CollectionKind::Skills,
            &spec("m", 4),
            embedder_b,
            60,
            0,
            Some(8),
            CancellationToken::new(),
        )
        .await
    } else {
        second
    };
    assert!(
        matches!(
            first,
            VectorReadiness::Ready | VectorReadiness::ReadyMissing { .. }
        ),
        "{first:?}"
    );
    assert!(
        matches!(
            second,
            VectorReadiness::Ready | VectorReadiness::ReadyMissing { .. }
        ),
        "{second:?}"
    );
    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    assert!(
        idx.collection_state(CollectionKind::Skills)
            .unwrap()
            .pending_json
            .trim()
            .is_empty(),
        "loser must not leave pending_json on a completed collection"
    );
    assert_eq!(idx.vec_count(CollectionKind::Skills), 1);
}

#[tokio::test]
async fn search_knn_never_returns_ghost_before_prune_or_under_contention() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    idx.replace_inventory(
        CollectionKind::CallableAgents,
        1,
        &[item("explore", "explore", "explorer")],
    )
    .unwrap();
    drop(idx);

    let embedder: Option<Arc<dyn EmbeddingProvider>> =
        Some(Arc::new(MockEmbeddingProvider { dimensions: 4 }));
    let ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder.clone(),
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(ready, VectorReadiness::Ready), "{ready:?}");
    let agents_ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::CallableAgents,
        &spec("m", 4),
        embedder,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(agents_ready, VectorReadiness::Ready),
        "{agents_ready:?}"
    );

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let query = [0.1f32, 0.2, 0.3, 0.4];
    idx.db()
        .execute(
            "INSERT OR REPLACE INTO skills_vec(item_id, embedding) VALUES (?1, ?2)",
            rusqlite::params!["ghost", embedding_bytes(&query)],
        )
        .unwrap();
    idx.db()
        .execute(
            "INSERT OR REPLACE INTO skills_vec(item_id, embedding) VALUES (?1, ?2)",
            rusqlite::params!["explore", embedding_bytes(&query)],
        )
        .unwrap();
    assert_eq!(idx.item_count(CollectionKind::Skills), 1);
    assert!(idx.vec_count(CollectionKind::Skills) >= 3);

    let knn = idx.search_knn(CollectionKind::Skills, &query, 8).unwrap();
    assert!(
        knn.iter()
            .all(|h| h.item_id != "ghost" && h.item_id != "explore"),
        "ghost and sibling ids must not leak through skills KNN before prune: {knn:?}"
    );
    assert!(
        knn.iter().any(|h| h.item_id == "fmt"),
        "live skills item must remain queryable: {knn:?}"
    );

    idx.db().execute_batch("BEGIN IMMEDIATE;").unwrap();
    assert_eq!(
        idx.prune_orphan_vector_rows(CollectionKind::Skills),
        0,
        "nested BEGIN IMMEDIATE must fail closed under prune contention"
    );
    let knn = idx.search_knn(CollectionKind::Skills, &query, 8).unwrap();
    assert!(
        knn.iter()
            .all(|h| h.item_id != "ghost" && h.item_id != "explore"),
        "ghost must stay absent from KNN under prune contention: {knn:?}"
    );
    assert!(
        matches!(
            compatible_readiness(&idx, CollectionKind::Skills),
            VectorReadiness::Pending { owned: false }
        ),
        "unsafe extras must not report Ready while prune cannot run"
    );
    idx.db().execute_batch("COMMIT;").unwrap();

    let knn = idx.search_knn(CollectionKind::Skills, &query, 8).unwrap();
    assert!(
        knn.iter().all(|h| h.item_id != "ghost"),
        "ghost must stay absent after the contended txn commits: {knn:?}"
    );
    assert_eq!(idx.vec_count(CollectionKind::CallableAgents), 1);
}

#[tokio::test]
async fn search_knn_join_drops_closer_ghosts_and_still_returns_live_k() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    drop(idx);
    let embedder: Option<Arc<dyn EmbeddingProvider>> =
        Some(Arc::new(MockEmbeddingProvider { dimensions: 4 }));
    let ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(ready, VectorReadiness::Ready), "{ready:?}");

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let query = [0.1f32, 0.2, 0.3, 0.4];
    for ghost in ["ghost-a", "ghost-b", "ghost-c"] {
        idx.db()
            .execute(
                "INSERT OR REPLACE INTO skills_vec(item_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![ghost, embedding_bytes(&query)],
            )
            .unwrap();
    }
    assert!(idx.vec_count(CollectionKind::Skills) >= 4);

    let knn = idx.search_knn(CollectionKind::Skills, &query, 1).unwrap();
    assert!(
        knn.iter()
            .all(|h| h.item_id != "ghost-a" && h.item_id != "ghost-b" && h.item_id != "ghost-c"),
        "closer ghost ids must not be KNN-queryable even at k=1: {knn:?}"
    );
    assert_eq!(
        knn.len(),
        1,
        "live item must still fill k after JOIN-filtering closer ghosts: {knn:?}"
    );
    assert_eq!(knn[0].item_id, "fmt");
}

#[tokio::test]
async fn compatible_readiness_ghost_plus_hash_changed_item_is_not_ready_under_contention() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    drop(idx);
    let embedder: Option<Arc<dyn EmbeddingProvider>> =
        Some(Arc::new(MockEmbeddingProvider { dimensions: 4 }));
    let ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(ready, VectorReadiness::Ready), "{ready:?}");

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    let query = [0.1f32, 0.2, 0.3, 0.4];
    idx.db()
        .execute(
            "INSERT OR REPLACE INTO skills_vec(item_id, embedding) VALUES (?1, ?2)",
            rusqlite::params!["ghost", embedding_bytes(&query)],
        )
        .unwrap();
    idx.replace_inventory(
        CollectionKind::Skills,
        2,
        &[item("fmt", "rust-format", "formats rust files")],
    )
    .unwrap();
    assert_eq!(idx.item_count(CollectionKind::Skills), 1);
    assert_eq!(
        idx.vec_count(CollectionKind::Skills),
        1,
        "hash-change deletes the live vec row; the injected ghost keeps counts equal"
    );

    idx.db().execute_batch("BEGIN IMMEDIATE;").unwrap();
    assert_eq!(
        idx.prune_orphan_vector_rows(CollectionKind::Skills),
        0,
        "nested BEGIN IMMEDIATE must fail closed under prune contention"
    );
    assert!(
        matches!(
            compatible_readiness(&idx, CollectionKind::Skills),
            VectorReadiness::Pending { owned: false }
        ),
        "a ghost plus a hash-changed live item must not report Ready on COUNT equality"
    );
    let knn = idx.search_knn(CollectionKind::Skills, &query, 8).unwrap();
    assert!(
        knn.iter().all(|h| h.item_id != "ghost"),
        "ghost must stay absent from KNN under prune contention: {knn:?}"
    );
    idx.db().execute_batch("COMMIT;").unwrap();

    assert!(
        matches!(
            compatible_readiness(&idx, CollectionKind::Skills),
            VectorReadiness::ReadyMissing { missing: 1 }
        ),
        "after prune can run, the hash-changed live item must stay ReadyMissing"
    );
    let knn = idx.search_knn(CollectionKind::Skills, &query, 8).unwrap();
    assert!(
        knn.iter().all(|h| h.item_id != "ghost"),
        "pruned ghost must stay absent from KNN: {knn:?}"
    );
}

#[tokio::test]
async fn compatible_readiness_compensating_ghost_and_missing_live_is_pending_under_contention() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    drop(idx);
    let embedder: Option<Arc<dyn EmbeddingProvider>> =
        Some(Arc::new(MockEmbeddingProvider { dimensions: 4 }));
    let ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(ready, VectorReadiness::Ready), "{ready:?}");

    let idx = MetadataIndex::open_or_create(&db_path).unwrap();
    idx.replace_inventory(
        CollectionKind::Skills,
        2,
        &[
            item("fmt", "rust-format", "formats rust"),
            item("lint", "rust-lint", "lints rust"),
        ],
    )
    .unwrap();
    let query = [0.1f32, 0.2, 0.3, 0.4];
    idx.db()
        .execute(
            "INSERT OR REPLACE INTO skills_vec(item_id, embedding) VALUES (?1, ?2)",
            rusqlite::params!["ghost", embedding_bytes(&query)],
        )
        .unwrap();
    assert_eq!(idx.item_count(CollectionKind::Skills), 2);
    assert_eq!(
        idx.vec_count(CollectionKind::Skills),
        2,
        "one live vec plus one ghost compensates COUNT(items)"
    );

    idx.db().execute_batch("BEGIN IMMEDIATE;").unwrap();
    assert_eq!(
        idx.prune_orphan_vector_rows(CollectionKind::Skills),
        0,
        "nested BEGIN IMMEDIATE must fail closed under prune contention"
    );
    assert!(
        matches!(
            compatible_readiness(&idx, CollectionKind::Skills),
            VectorReadiness::Pending { owned: false }
        ),
        "compensating ghost extras must not report Ready"
    );
    let knn = idx.search_knn(CollectionKind::Skills, &query, 8).unwrap();
    assert!(
        knn.iter().all(|h| h.item_id != "ghost"),
        "ghost must stay absent from KNN under prune contention: {knn:?}"
    );
    assert!(
        knn.iter().any(|h| h.item_id == "fmt"),
        "live embedded item must remain queryable: {knn:?}"
    );
    idx.db().execute_batch("COMMIT;").unwrap();

    assert!(
        matches!(
            compatible_readiness(&idx, CollectionKind::Skills),
            VectorReadiness::ReadyMissing { missing: 1 }
        ),
        "after prune can run, the unembedded live item must stay ReadyMissing"
    );
    let knn = idx.search_knn(CollectionKind::Skills, &query, 8).unwrap();
    assert!(
        knn.iter().all(|h| h.item_id != "ghost"),
        "pruned ghost must stay absent from KNN: {knn:?}"
    );
}

#[tokio::test]
async fn upsert_embedding_lost_item_does_not_create_ghost_under_contention() {
    let tmp = TempDir::new().unwrap();
    let (db_path, idx) = open(&tmp);
    idx.replace_inventory(
        CollectionKind::Skills,
        1,
        &[item("fmt", "rust-format", "formats rust")],
    )
    .unwrap();
    drop(idx);
    let embedder: Option<Arc<dyn EmbeddingProvider>> =
        Some(Arc::new(MockEmbeddingProvider { dimensions: 4 }));
    let ready = ensure_collection_vectors_ready(
        &db_path,
        CollectionKind::Skills,
        &spec("m", 4),
        embedder,
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(ready, VectorReadiness::Ready), "{ready:?}");

    struct PauseGuard;
    impl Drop for PauseGuard {
        fn drop(&mut self) {
            UPSERT_BEFORE_TXN_PAUSE.store(false, std::sync::atomic::Ordering::SeqCst);
            UPSERT_BEFORE_TXN_REACHED.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _pause_guard = PauseGuard;
    UPSERT_BEFORE_TXN_REACHED.store(false, std::sync::atomic::Ordering::SeqCst);
    UPSERT_BEFORE_TXN_PAUSE.store(true, std::sync::atomic::Ordering::SeqCst);

    let idx_upsert = MetadataIndex::open_or_create(&db_path).unwrap();
    let idx_delete = MetadataIndex::open_or_create(&db_path).unwrap();
    let vector = [0.9f32, 0.8, 0.7, 0.6];
    let handle = std::thread::spawn(move || {
        idx_upsert.upsert_embedding(CollectionKind::Skills, "fmt", &vector)
    });

    let started = std::time::Instant::now();
    while !UPSERT_BEFORE_TXN_REACHED.load(std::sync::atomic::Ordering::SeqCst) {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "upsert thread never reached the pre-txn gate"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    let removed = idx_delete
        .replace_inventory(CollectionKind::Skills, 99, &[])
        .unwrap();
    assert_eq!(removed.removed, 1);
    assert_eq!(idx_delete.item_count(CollectionKind::Skills), 0);

    UPSERT_BEFORE_TXN_PAUSE.store(false, std::sync::atomic::Ordering::SeqCst);
    let err = handle.join().expect("upsert thread");
    assert!(
        matches!(err, Err(MetadataIndexError::InvalidItem(_))),
        "disappeared item must roll back the embedding write: {err:?}"
    );

    assert_eq!(idx_delete.item_count(CollectionKind::Skills), 0);
    let knn = idx_delete
        .search_knn(CollectionKind::Skills, &vector, 8)
        .unwrap();
    assert!(
        knn.iter()
            .all(|h| h.item_id != "fmt" && h.item_id != "ghost"),
        "replace_inventory deletion must not race a ghost vec row: {knn:?}"
    );
    assert_eq!(idx_delete.vec_count(CollectionKind::Skills), 0);
}

#[test]
fn rebuild_claim_pid_is_parsed_not_prefix_matched() {
    assert!(
        !claim_owned_by_pid("12:100", 1),
        "PID 1 must not steal owner 12"
    );
    assert!(
        !claim_owned_by_pid("123:100", 12),
        "PID 12 must not steal owner 123"
    );
    assert!(
        claim_owned_by_pid("1:100", 1),
        "exact PID 1 must match owner 1"
    );
    assert!(
        claim_owned_by_pid("12:999", 12),
        "exact PID 12 must match owner 12"
    );
    assert!(
        !claim_owned_by_pid("12", 12),
        "claim without a colon is never same-process"
    );
    assert!(!claim_owned_by_pid("", 1));
    assert!(!claim_owned_by_pid("abc:1", 1));

    let now = 1_700_000_000i64;
    assert!(
        !claim_is_reclaimable("12:100", now, now, 3600, 1),
        "fresh owner 12 is not reclaimable by PID 1"
    );
    assert!(
        !claim_is_reclaimable("123:100", now, now, 3600, 12),
        "fresh owner 123 is not reclaimable by PID 12"
    );
    assert!(
        claim_is_reclaimable("12:100", now, now, 3600, 12),
        "same-PID owner 12 must reclaim"
    );
    assert!(
        claim_is_reclaimable("1:100", now, now, 3600, 1),
        "same-PID owner 1 must reclaim"
    );
    assert!(
        claim_is_reclaimable("123:100", now - 4000, now, 3600, 12),
        "stale timestamp still yields to a different PID"
    );
    assert!(claim_is_reclaimable("", now, now, 3600, 1));
}

#[test]
fn try_claim_rebuild_same_pid_reclaim_and_prefix_owners() {
    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    let pid = std::process::id();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut pending = ensure_pending(&idx, CollectionKind::Skills, "fp", "test").unwrap();
    pending.claim = format!("{pid}:1");
    pending.claimed_at = now;
    pending.status = "running".into();
    idx.db()
        .execute(
            "UPDATE collections SET pending_json = ?1 WHERE name = ?2",
            rusqlite::params![pending.to_json(), CollectionKind::Skills.as_str()],
        )
        .unwrap();
    let mut same = pending_state_required(&idx, CollectionKind::Skills);
    assert!(
        try_claim_rebuild(&idx, CollectionKind::Skills, &mut same, 3600),
        "valid same-PID reclaim must succeed while the timestamp is still fresh"
    );

    let mut prefix_owner = pending_state_required(&idx, CollectionKind::Skills);
    prefix_owner.claim = format!("{pid}2:{now}");
    prefix_owner.claimed_at = now;
    prefix_owner.status = "running".into();
    idx.db()
        .execute(
            "UPDATE collections SET pending_json = ?1 WHERE name = ?2",
            rusqlite::params![prefix_owner.to_json(), CollectionKind::Skills.as_str()],
        )
        .unwrap();
    let mut stolen = pending_state_required(&idx, CollectionKind::Skills);
    assert!(
        !try_claim_rebuild(&idx, CollectionKind::Skills, &mut stolen, 3600),
        "owner whose PID string only has ours as a prefix must not be stolen"
    );

    let mut owner_twelve = pending_state_required(&idx, CollectionKind::Skills);
    owner_twelve.claim = format!("12:{now}");
    owner_twelve.claimed_at = now;
    owner_twelve.status = "running".into();
    idx.db()
        .execute(
            "UPDATE collections SET pending_json = ?1 WHERE name = ?2",
            rusqlite::params![owner_twelve.to_json(), CollectionKind::Skills.as_str()],
        )
        .unwrap();
    let mut one_vs_twelve = pending_state_required(&idx, CollectionKind::Skills);
    if pid != 12 {
        assert!(
            !try_claim_rebuild(&idx, CollectionKind::Skills, &mut one_vs_twelve, 3600),
            "fresh owner 12 must not be stolen by PID {pid}"
        );
    }

    let mut owner_123 = pending_state_required(&idx, CollectionKind::Skills);
    owner_123.claim = format!("123:{now}");
    owner_123.claimed_at = now;
    owner_123.status = "running".into();
    idx.db()
        .execute(
            "UPDATE collections SET pending_json = ?1 WHERE name = ?2",
            rusqlite::params![owner_123.to_json(), CollectionKind::Skills.as_str()],
        )
        .unwrap();
    let mut twelve_vs_123 = pending_state_required(&idx, CollectionKind::Skills);
    if pid != 123 {
        assert!(
            !try_claim_rebuild(&idx, CollectionKind::Skills, &mut twelve_vs_123, 3600),
            "fresh owner 123 must not be stolen by PID {pid}"
        );
    }

    let mut stale = pending_state_required(&idx, CollectionKind::Skills);
    stale.claim = format!("123:{now}");
    stale.claimed_at = 1;
    stale.status = "running".into();
    idx.db()
        .execute(
            "UPDATE collections SET pending_json = ?1 WHERE name = ?2",
            rusqlite::params![stale.to_json(), CollectionKind::Skills.as_str()],
        )
        .unwrap();
    let mut stale_claim = pending_state_required(&idx, CollectionKind::Skills);
    assert!(
        try_claim_rebuild(&idx, CollectionKind::Skills, &mut stale_claim, 3600),
        "stale timestamp must remain reclaimable by a different PID"
    );
}

#[test]
fn collection_pending_to_json_escapes_quotes_and_backslashes() {
    let tmp = TempDir::new().unwrap();
    let (_, idx) = open(&tmp);
    let pending = super::rebuild::CollectionPending {
        id: r#"id"quote\slash"#.into(),
        intended: r#"fp"x\y"#.into(),
        status: r#"pend"ing"#.into(),
        claim: r#"12:a"b\c"#.into(),
        claimed_at: 42,
        reason: r#"why "now" and \path"#.into(),
        last_attempt_at: 7,
    };
    let json = pending.to_json();
    serde_json::from_str::<serde_json::Value>(&json)
        .expect("to_json must emit valid JSON for quote and backslash values");
    idx.db()
        .execute(
            "UPDATE collections SET pending_json = ?1 WHERE name = ?2",
            rusqlite::params![json, CollectionKind::Skills.as_str()],
        )
        .unwrap();
    let stored = pending_state_required(&idx, CollectionKind::Skills);
    assert_eq!(stored, pending);
}
