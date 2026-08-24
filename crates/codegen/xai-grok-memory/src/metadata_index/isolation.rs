//! Memory vs Prime database isolation and leak scans.

use std::fs;
use std::path::Path;

use tempfile::TempDir;
use xai_grok_config_types::MemoryIndexConfig;

use super::{CollectionKind, MetadataIndex, MetadataItem, metadata_index_path};
use crate::index::{MemoryIndex, init_sqlite_vec};
use crate::storage::MemoryStorage;
use crate::workspace_identity::workspace_storage_identity;

const MEMORY_ONLY: &str = "MEMORY_ONLY_PROMPT_FIXTURE";
const PRIME_ONLY: &str = "prime-format";
const FORBIDDEN: &[&str] = &[
    "sk-live-SUPERSECRETVALUE",
    "BEGIN PRIVATE",
    "YOU ARE A HELPFUL ASSISTANT",
    "0.39215687",
    "SECRET-BODY",
    "raw provider error",
];

fn scan_bytes(path: &Path) -> String {
    String::from_utf8_lossy(&fs::read(path).unwrap_or_default()).to_ascii_lowercase()
}

#[test]
fn memory_and_prime_databases_remain_isolated_and_secret_free() {
    init_sqlite_vec();
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("grok-home");
    let cwd = tmp.path().join("workspace");
    fs::create_dir_all(&cwd).unwrap();
    let identity = workspace_storage_identity(&cwd);

    let memory_root = home.join("memory");
    let memory_ws = memory_root.join(&identity);
    fs::create_dir_all(&memory_ws).unwrap();
    let memory_md = memory_ws.join("MEMORY.md");
    fs::write(
        &memory_md,
        format!("# Memory\n\n{MEMORY_ONLY}\nsk-live-SUPERSECRETVALUE\n"),
    )
    .unwrap();
    let memory_db = memory_ws.join("index.sqlite");
    let storage = MemoryStorage::with_paths(memory_root.clone(), memory_ws.clone());
    let mut memory =
        MemoryIndex::open_or_create(&memory_db, storage, MemoryIndexConfig::default(), 4).unwrap();
    memory
        .reindex_file(&memory_md, "workspace")
        .expect("reindex memory");
    drop(memory);

    let prime_db = metadata_index_path(&home, &identity);
    fs::create_dir_all(prime_db.parent().unwrap()).unwrap();
    let prime = MetadataIndex::open_or_create(&prime_db).unwrap();
    prime
        .replace_inventory(
            CollectionKind::Skills,
            1,
            &[MetadataItem::new(PRIME_ONLY, PRIME_ONLY, "formats rust sources", "").unwrap()],
        )
        .unwrap();
    prime
        .replace_inventory(
            CollectionKind::CallableAgents,
            1,
            &[MetadataItem::new("explore", "explore", "read-only explorer", "").unwrap()],
        )
        .unwrap();
    let prime_cells = prime.text_cells().unwrap().join("\n");
    drop(prime);

    assert_ne!(memory_db, prime_db);
    assert!(memory_db.ends_with("index.sqlite"));
    assert!(prime_db.ends_with("metadata.sqlite"));
    assert!(memory_db.to_string_lossy().contains("/memory/"));
    assert!(prime_db.to_string_lossy().contains("/indexes/prime/"));

    let memory_bytes = scan_bytes(&memory_db);
    let prime_bytes = scan_bytes(&prime_db);
    let prime_lower = prime_cells.to_ascii_lowercase();

    assert!(
        memory_bytes.contains(&MEMORY_ONLY.to_ascii_lowercase()),
        "memory index must retain its own fixture text"
    );
    assert!(
        !prime_bytes.contains(&MEMORY_ONLY.to_ascii_lowercase()),
        "prime index must not contain memory fixture text"
    );
    assert!(
        !prime_lower.contains(&MEMORY_ONLY.to_ascii_lowercase()),
        "prime text cells must not contain memory fixture text"
    );
    assert!(
        prime_lower.contains(PRIME_ONLY),
        "prime index must retain its own skill name"
    );
    assert!(
        !memory_bytes.contains(PRIME_ONLY),
        "memory index must not contain prime skill ids"
    );

    for token in FORBIDDEN {
        let needle = token.to_ascii_lowercase();
        assert!(!prime_bytes.contains(&needle), "prime db leaked {token}");
        assert!(!prime_lower.contains(&needle), "prime cells leaked {token}");
    }
    assert!(
        !prime_bytes.contains("/users/"),
        "prime db must not persist absolute paths"
    );
    assert!(
        !prime_lower.contains("fn main"),
        "prime db must not persist bodies"
    );
}
