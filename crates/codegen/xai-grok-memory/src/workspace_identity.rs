//! Shared workspace storage identity.
//!
//! Memory files and the disposable Prime metadata index must key durable
//! per-workspace state by the same identity so clones, worktrees, and copies
//! of one repository resolve to one storage directory. This module is the
//! single helper; do not add a separate workspace crate.

use std::path::Path;

/// Compute a human-friendly workspace directory name.
///
/// Format: `{slug}-{hash8}` where:
/// - `slug` is the repo or directory name, slugified (max 40 chars)
/// - `hash8` is 8 hex chars from blake3 for uniqueness
///
/// **Identity strategy:** Prefers git remote `org/repo` as the identity
/// source — all clones, worktrees, and copies of the same repository
/// resolve to the same directory regardless of filesystem path.
/// Falls back to filesystem path when not inside a git repo or when
/// no `origin` remote is configured.
///
/// The returned value is a directory component, never an absolute path.
pub fn workspace_storage_identity(cwd: &Path) -> String {
    let identity = extract_repo_identity(cwd);

    let (slug, hash_input) = match identity {
        Some(ref repo_id) => {
            let slug_source = repo_id.rsplit('/').next().unwrap_or(repo_id);
            (slugify(slug_source, 40), repo_id.as_str().to_string())
        }
        None => {
            // Windows-only, non-git cwds: dunce changes the hash input, so the old-form dir is orphaned until session gc() reaps it after max_age_days — accepted over an unverifiable rename migration (Unix unchanged).
            let canonical = dunce::canonicalize(cwd).unwrap_or_else(|_| {
                tracing::warn!(
                    path = %cwd.display(),
                    "could not canonicalize workspace path for memory hash; using raw path"
                );
                cwd.to_path_buf()
            });
            let dir_name = canonical
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace");
            (
                slugify(dir_name, 40),
                canonical.to_string_lossy().to_string(),
            )
        }
    };

    let slug = if slug.is_empty() { "workspace" } else { &slug };
    let hash = blake3::hash(hash_input.as_bytes());
    let hash8 = &hash.to_hex()[..8];

    format!("{slug}-{hash8}")
}

/// Compute the stable 16-hex workspace identity hash used for remote
/// collection naming (e.g. Milvus mirror collections).
///
/// Uses the same identity input as [`workspace_storage_identity`]: the
/// normalized git remote `org/repo` when the workspace has an `origin`
/// remote (so clones, worktrees, and copies of one repository share one
/// hash), otherwise the canonical filesystem path. The digest is blake3
/// truncated to 16 hex characters; its first 8 characters always equal the
/// `hash8` suffix of [`workspace_storage_identity`].
#[must_use]
pub fn workspace_identity_hash16(cwd: &Path) -> String {
    let hash_input = match extract_repo_identity(cwd) {
        Some(repo_id) => repo_id,
        None => dunce::canonicalize(cwd)
            .unwrap_or_else(|_| cwd.to_path_buf())
            .to_string_lossy()
            .into_owned(),
    };
    let hash = blake3::hash(hash_input.as_bytes());
    hash.to_hex()[..16].to_string()
}

/// Extract a normalized `org/repo` identifier from the git remote URL.
///
/// Uses `git2` to discover the repository from `cwd` and read the `origin`
/// remote URL. Returns `None` if not a git repo, no `origin` remote, or the
/// URL can't be normalized.
pub(crate) fn extract_repo_identity(cwd: &Path) -> Option<String> {
    let repo = git2::Repository::discover(cwd).ok()?;
    let remote = repo.find_remote("origin").ok()?;
    let url = remote.url()?;
    normalize_remote_url(url)
}

/// Normalize a git remote URL to `org/repo` form.
///
/// Strips protocol prefix, host, and trailing `.git`:
/// - `git@github.com:acme/widgets.git`       → `"acme/widgets"`
/// - `https://github.com/acme/widgets.git`   → `"acme/widgets"`
/// - `ssh://git@github.com/acme/widgets`     → `"acme/widgets"`
pub(crate) fn normalize_remote_url(url: &str) -> Option<String> {
    let path = if let Some(colon_pos) = url.find(':') {
        // SSH format: git@github.com:org/repo.git
        if url[..colon_pos].contains('@') && !url[..colon_pos].contains('/') {
            &url[colon_pos + 1..]
        } else {
            // HTTPS/SSH-with-scheme: https://github.com/org/repo.git
            url.split("//")
                .nth(1)
                .and_then(|after_scheme| after_scheme.split_once('/'))
                .map(|(_, path)| path)?
        }
    } else {
        return None;
    };

    let cleaned = path
        .trim_end_matches(".git")
        .trim_end_matches('/')
        .trim_start_matches('/');

    if cleaned.is_empty() || !cleaned.contains('/') {
        return None;
    }

    Some(cleaned.to_string())
}

/// Generate a URL-safe slug from a string (e.g., first user message).
///
/// - Lowercases
/// - Replaces non-alphanumeric chars with `-`
/// - Collapses consecutive dashes
/// - Truncates to `max_len` **characters** (not bytes)
/// - Strips leading/trailing `-`
pub fn slugify(input: &str, max_len: usize) -> String {
    let slug: String = input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse consecutive dashes
    let mut result = String::with_capacity(slug.len());
    let mut prev_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_dash {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }

    // Truncate by char count (safe for multi-byte), then trim dashes
    let truncated: String = result.chars().take(max_len).collect();
    truncated.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash16_is_stable_16_lowercase_hex_for_same_cwd() {
        let cwd = std::env::temp_dir().join("grok-mirror-hash16-stability");
        std::fs::create_dir_all(&cwd).unwrap();
        let a = workspace_identity_hash16(&cwd);
        let b = workspace_identity_hash16(&cwd);
        assert_eq!(a, b, "same cwd must hash identically");
        assert_eq!(a.len(), 16, "hash16 must be 16 chars: {a}");
        assert!(
            a.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "hash16 must be lowercase hex: {a}"
        );
    }

    #[test]
    fn hash16_prefix_matches_storage_identity_hash8() {
        // A temp dir outside any repo hashes its canonical path; the first
        // 8 chars of hash16 must equal the hash8 suffix of the storage
        // identity computed from the same input.
        let cwd = std::env::temp_dir().join("grok-mirror-hash16-prefix");
        std::fs::create_dir_all(&cwd).unwrap();
        let hash16 = workspace_identity_hash16(&cwd);
        let storage = workspace_storage_identity(&cwd);
        let hash8 = storage.rsplit('-').next().unwrap();
        assert_eq!(&hash16[..8], hash8);
    }

    #[test]
    fn hash16_tracks_this_repo_remote_identity() {
        // This crate lives in a git checkout with an `origin` remote; the
        // identity (and therefore the collection name) must be path- and
        // clone-stable, i.e. derived from `org/repo`, not the filesystem.
        let cwd = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        if extract_repo_identity(cwd).is_none() {
            return; // no origin remote in this checkout: nothing to assert
        }
        let hash16 = workspace_identity_hash16(cwd);
        let identity = extract_repo_identity(cwd).unwrap();
        let expected = blake3::hash(identity.as_bytes());
        assert_eq!(hash16, &expected.to_hex()[..16]);
    }
}
