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
