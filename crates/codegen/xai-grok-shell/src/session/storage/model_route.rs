//! Companion `model_route.json` + private `model_identity.meta` pair.
//!
//! Route provenance is **not** a public `Summary` field. It is stored as a
//! secret-free companion next to `summary.json`, bound via private meta
//! (pair_id + summary digest). Leave never adopts a mismatched companion.
//!
//! Identity I/O uses owner-checked files under the session directory (no
//! symlink follow). Transaction journal: `model_identity.txn`.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use xai_grok_models::ModelRouteProvenance;

use crate::session::persistence::Summary;

pub(crate) const MODEL_ROUTE_FILE: &str = "model_route.json";
pub(crate) const MODEL_IDENTITY_META: &str = "model_identity.meta";
pub(crate) const MODEL_IDENTITY_TXN: &str = "model_identity.txn";
pub(crate) const MODEL_IDENTITY_LOCK: &str = "model_identity.lock";
const MAX_META_BYTES: usize = 4096;
const META_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct IdentityMeta {
    version: u32,
    pair_id: String,
    canonical_model: String,
    summary_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    companion_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxnMarker {
    version: u32,
    summary_tmp: Option<String>,
    companion_tmp: Option<String>,
    meta_tmp: Option<String>,
    new_summary_sha: Option<String>,
    previous_summary_sha: Option<String>,
}

fn session_file(session_dir: &Path, name: &str) -> PathBuf {
    session_dir.join(name)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn read_file_nofollow(path: &Path) -> io::Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "refusing symlink identity file",
        ));
    }
    if !meta.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "identity path is not a regular file",
        ));
    }
    fs::read(path)
}

fn write_bytes_atomic_nofollow(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "identity path has no parent")
    })?;
    let tmp = parent.join(format!(
        ".{}.tmp.{}",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("id"),
        std::process::id()
    ));
    {
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn lock_identity(session_dir: &Path) -> io::Result<File> {
    let lock_path = session_file(session_dir, MODEL_IDENTITY_LOCK);
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    use fs2::FileExt;
    file.lock_exclusive()?;
    Ok(file)
}

/// Load companion provenance if present and bound to the summary.
/// Old sessions without companion/meta load successfully (no rewrite).
pub fn load_route_companion(
    session_dir: &Path,
    summary: &Summary,
) -> io::Result<Option<ModelRouteProvenance>> {
    let companion_path = session_file(session_dir, MODEL_ROUTE_FILE);
    let meta_path = session_file(session_dir, MODEL_IDENTITY_META);
    let companion_exists = companion_path.exists();
    let meta_exists = meta_path.exists();
    if !companion_exists && !meta_exists {
        return Ok(None);
    }
    if companion_exists != meta_exists {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incomplete model identity pair (companion/meta mismatch)",
        ));
    }
    let companion_bytes = read_file_nofollow(&companion_path)?;
    let meta_bytes = read_file_nofollow(&meta_path)?;
    if meta_bytes.len() > MAX_META_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "model_identity.meta too large",
        ));
    }
    let meta: IdentityMeta = serde_json::from_slice(&meta_bytes).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("malformed model_identity.meta: {e}"),
        )
    })?;
    if meta.version != META_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported model_identity.meta version",
        ));
    }
    if meta.canonical_model != summary.current_model_id.0.as_ref() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "model_route.json does not match summary current_model_id",
        ));
    }
    // Digest the on-disk summary bytes (not a re-serialize) so pretty/compact
    // encoding differences cannot false-fail a valid pair.
    let summary_path = session_file(session_dir, "summary.json");
    if summary_path.exists() {
        let summary_bytes = read_file_nofollow(&summary_path)?;
        let digest = sha256_hex(&summary_bytes);
        if meta.summary_sha256 != digest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "model identity summary digest mismatch",
            ));
        }
    }
    let companion: ModelRouteProvenance =
        serde_json::from_slice(&companion_bytes).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("malformed model_route.json: {e}"),
            )
        })?;
    if companion.pair_id.as_deref() != Some(meta.pair_id.as_str()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "model route pair_id mismatch",
        ));
    }
    if let Some(expected) = &meta.companion_sha256 {
        let actual = sha256_hex(&companion_bytes);
        if actual != *expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "model_route companion digest mismatch",
            ));
        }
    }
    Ok(Some(companion))
}

/// Commit summary + optional companion as one identity transaction.
/// `force_new_pair` / Set installs a fresh pair; Leave with mismatch fails closed.
pub fn commit_summary_and_companion(
    session_dir: &Path,
    summary: &Summary,
    companion: Option<&ModelRouteProvenance>,
    leave_on_mismatch: bool,
) -> io::Result<()> {
    let _lock = lock_identity(session_dir)?;
    recover_identity_txn(session_dir)?;

    let summary_path = session_file(session_dir, "summary.json");
    let companion_path = session_file(session_dir, MODEL_ROUTE_FILE);
    let meta_path = session_file(session_dir, MODEL_IDENTITY_META);

    let previous_summary_bytes = if summary_path.exists() {
        Some(read_file_nofollow(&summary_path)?)
    } else {
        None
    };

    if leave_on_mismatch && companion.is_none() {
        // Leave: validate existing pair against previous summary, then update digest only.
        if companion_path.exists() || meta_path.exists() {
            if let Some(prev) = &previous_summary_bytes {
                let prev_summary: Summary = serde_json::from_slice(prev).map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("summary: {e}"))
                })?;
                // Fail closed if existing pair is invalid vs previous committed summary.
                let _ = load_route_companion(session_dir, &prev_summary)?;
            }
        }
        let summary_bytes = serde_json::to_vec_pretty(summary).map_err(io::Error::other)?;
        // Preserve companion/meta bytes; only rewrite summary + meta digest.
        if meta_path.exists() {
            let meta_bytes = read_file_nofollow(&meta_path)?;
            let mut meta: IdentityMeta = serde_json::from_slice(&meta_bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("meta: {e}")))?;
            meta.summary_sha256 = sha256_hex(&summary_bytes);
            write_bytes_atomic_nofollow(&summary_path, &summary_bytes)?;
            write_bytes_atomic_nofollow(
                &meta_path,
                &serde_json::to_vec_pretty(&meta).map_err(io::Error::other)?,
            )?;
        } else {
            write_bytes_atomic_nofollow(&summary_path, &summary_bytes)?;
        }
        return Ok(());
    }

    let summary_bytes = serde_json::to_vec_pretty(summary).map_err(io::Error::other)?;
    let summary_sha = sha256_hex(&summary_bytes);

    let (companion_bytes, pair_id, companion_sha) = if let Some(c) = companion {
        let pair = c
            .pair_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let mut c = c.clone();
        if c.pair_id.is_none() {
            c = c
                .with_pair_id(&pair)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        }
        if c.canonical_model.is_none() {
            c.canonical_model = Some(summary.current_model_id.0.to_string());
        }
        let bytes = serde_json::to_vec_pretty(&c).map_err(io::Error::other)?;
        let sha = sha256_hex(&bytes);
        (Some(bytes), pair, Some(sha))
    } else {
        (None, uuid::Uuid::new_v4().to_string(), None)
    };

    let meta = IdentityMeta {
        version: META_VERSION,
        pair_id: pair_id.clone(),
        canonical_model: summary.current_model_id.0.to_string(),
        summary_sha256: summary_sha.clone(),
        companion_sha256: companion_sha,
    };
    let meta_bytes = serde_json::to_vec_pretty(&meta).map_err(io::Error::other)?;

    let txn_path = session_file(session_dir, MODEL_IDENTITY_TXN);
    let marker = TxnMarker {
        version: 1,
        summary_tmp: Some("summary.json.tmp".into()),
        companion_tmp: companion_bytes
            .as_ref()
            .map(|_| "model_route.json.tmp".into()),
        meta_tmp: Some("model_identity.meta.tmp".into()),
        new_summary_sha: Some(summary_sha),
        previous_summary_sha: previous_summary_bytes.as_ref().map(|b| sha256_hex(b)),
    };
    write_bytes_atomic_nofollow(
        &txn_path,
        &serde_json::to_vec_pretty(&marker).map_err(io::Error::other)?,
    )?;

    write_bytes_atomic_nofollow(&summary_path, &summary_bytes)?;
    if let Some(bytes) = &companion_bytes {
        write_bytes_atomic_nofollow(&companion_path, bytes)?;
    } else if companion_path.exists() {
        let _ = fs::remove_file(&companion_path);
    }
    write_bytes_atomic_nofollow(&meta_path, &meta_bytes)?;
    let _ = fs::remove_file(&txn_path);
    Ok(())
}

fn recover_identity_txn(session_dir: &Path) -> io::Result<()> {
    let txn_path = session_file(session_dir, MODEL_IDENTITY_TXN);
    if !txn_path.exists() {
        return Ok(());
    }
    // Incomplete txn: fail closed by removing marker after best-effort cleanup of temps.
    for name in [
        "summary.json.tmp",
        "model_route.json.tmp",
        "model_identity.meta.tmp",
    ] {
        let p = session_file(session_dir, name);
        if p.exists() {
            let _ = fs::remove_file(&p);
        }
    }
    let _ = fs::remove_file(&txn_path);
    Ok(())
}

/// Clear companion when model changes without new provenance.
pub fn clear_route_companion(session_dir: &Path) -> io::Result<()> {
    let _lock = lock_identity(session_dir)?;
    for name in [MODEL_ROUTE_FILE, MODEL_IDENTITY_META, MODEL_IDENTITY_TXN] {
        let p = session_file(session_dir, name);
        if p.exists() {
            let meta = fs::symlink_metadata(&p)?;
            if meta.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "refusing to unlink symlink identity file",
                ));
            }
            fs::remove_file(&p)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::info::Info;
    use agent_client_protocol as acp;
    use chrono::Utc;
    use tempfile::tempdir;
    use xai_grok_models::{ModelRouteProvenance, UpstreamModelId};

    fn sample_summary(model: &str) -> Summary {
        Summary {
            info: Info {
                id: acp::SessionId::new("s1"),
                cwd: "/tmp".into(),
            },
            cwd_generation: 0,
            previous_cwd: None,
            pending_cwd_switch_reminder: None,
            cwd_switch_bookkeeping_generation: 0,
            session_summary: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            num_messages: 0,
            num_chat_messages: 0,
            current_model_id: acp::ModelId::new(model),
            parent_session_id: None,
            forked_at: None,
            collection_id: None,
            next_trace_turn: 0,
            chat_format_version: 1,
            prompt_display_cwd: None,
            session_kind: None,
            fork_context_source: None,
            fork_parent_prompt_id: None,
            inherited_prefix_len: None,
            hidden: None,
            source_workspace_dir: None,
            git_root_dir: None,
            git_remotes: Vec::new(),
            head_commit: None,
            head_branch: None,
            request_id: None,
            grok_home: None,
            last_active_at: None,
            generated_title: None,
            title_is_manual: false,
            worktree_label: None,
            agent_name: None,
            sandbox_profile: None,
            reasoning_effort: None,
            execution_backend: crate::agent::execution_backend::ExecutionBackend::NativeInference,
            external_runtime: None,
        }
    }

    fn provenance(canonical: &str) -> ModelRouteProvenance {
        let upstream = UpstreamModelId::new("gpt-4o").unwrap();
        ModelRouteProvenance::new(
            "openai",
            Some("01234567-89ab-cdef-0123-456789abcdef"),
            Some("openai"),
            Some("openai_platform"),
            &upstream,
            2,
        )
        .unwrap()
        .with_canonical_model(&xai_grok_models::CanonicalModelId::new(canonical).unwrap())
        .with_pair_id("pair-token-aaaaaaaaaaaaaaaa")
        .unwrap()
    }

    #[test]
    fn write_is_0600_and_round_trips() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("sess");
        fs::create_dir_all(&session).unwrap();
        let summary = sample_summary("openai-gpt-4o");
        let prov = provenance("openai-gpt-4o");
        commit_summary_and_companion(&session, &summary, Some(&prov), false).unwrap();
        let loaded = load_route_companion(&session, &summary).unwrap().unwrap();
        assert_eq!(loaded.upstream_model, "gpt-4o");
        assert_eq!(
            loaded.pair_id.as_deref(),
            Some("pair-token-aaaaaaaaaaaaaaaa")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(session.join(MODEL_ROUTE_FILE))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn leave_mismatch_fails_closed_without_adoption() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("sess");
        fs::create_dir_all(&session).unwrap();
        let summary = sample_summary("openai-gpt-4o");
        let prov = provenance("openai-gpt-4o");
        commit_summary_and_companion(&session, &summary, Some(&prov), false).unwrap();
        // Corrupt meta digest.
        let meta_path = session.join(MODEL_IDENTITY_META);
        let mut meta: IdentityMeta =
            serde_json::from_slice(&fs::read(&meta_path).unwrap()).unwrap();
        meta.summary_sha256 = "0".repeat(64);
        fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();
        let mut next = summary.clone();
        next.num_messages = 1;
        let err = commit_summary_and_companion(&session, &next, None, true).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn leave_valid_updates_digest_preserves_pair() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("sess");
        fs::create_dir_all(&session).unwrap();
        let summary = sample_summary("openai-gpt-4o");
        let prov = provenance("openai-gpt-4o");
        commit_summary_and_companion(&session, &summary, Some(&prov), false).unwrap();
        let before = fs::read(session.join(MODEL_ROUTE_FILE)).unwrap();
        let mut next = summary.clone();
        next.num_messages = 3;
        commit_summary_and_companion(&session, &next, None, true).unwrap();
        let after = fs::read(session.join(MODEL_ROUTE_FILE)).unwrap();
        assert_eq!(before, after);
        let loaded = load_route_companion(&session, &next).unwrap().unwrap();
        assert_eq!(
            loaded.pair_id.as_deref(),
            Some("pair-token-aaaaaaaaaaaaaaaa")
        );
    }

    #[test]
    fn same_canonical_stale_pair_id_fails_closed_on_load() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("sess");
        fs::create_dir_all(&session).unwrap();
        let summary = sample_summary("openai-gpt-4o");
        let prov = provenance("openai-gpt-4o");
        commit_summary_and_companion(&session, &summary, Some(&prov), false).unwrap();
        let mut companion: ModelRouteProvenance =
            serde_json::from_slice(&fs::read(session.join(MODEL_ROUTE_FILE)).unwrap()).unwrap();
        companion.pair_id = Some("different-pair-token-bbbbbbbb".into());
        fs::write(
            session.join(MODEL_ROUTE_FILE),
            serde_json::to_vec(&companion).unwrap(),
        )
        .unwrap();
        let err = load_route_companion(&session, &summary).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn summary_base_literal_compiles_without_route_field() {
        // Exhaustive Summary construction without model_route_pair_id.
        let _ = sample_summary("m");
    }

    #[test]
    fn old_session_without_companion_loads() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("sess");
        fs::create_dir_all(&session).unwrap();
        let summary = sample_summary("grok-4.5");
        assert!(load_route_companion(&session, &summary).unwrap().is_none());
    }
}
