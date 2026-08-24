//! Owner-only staging, complete-directory validation, and atomic skill publish.
//!
//! A failed publish never leaves a partial active skill directory. Diagnostics
//! never include absolute paths, bodies, or raw parser text.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use super::evals::{EvalSchemaError, load_eval_suite_from_dir};
use super::inventory::SkillIdentity;
use super::spec::SKILL_MD_FILE_NAME;
use super::status::SkillHealthStatus;
use super::validator::{StrictSkillOutcome, validate_strict_skill_dir};
use crate::implementations::skills::types::SkillScope;

pub const MAX_SKILL_TREE_FILES: usize = 64;
pub const MAX_SKILL_FILE_BYTES: u64 = 256 * 1024;
pub const MAX_SKILL_TREE_BYTES: u64 = 1024 * 1024;
const STAGING_DIR_NAME: &str = ".staging";
static STAGING_SEQ: AtomicU64 = AtomicU64::new(0);
static PUBLISH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn publish_lock() -> &'static Mutex<()> {
    PUBLISH_LOCK.get_or_init(|| Mutex::new(()))
}

/// Destination roots the publisher may write into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishScope {
    Project,
    User,
}

impl PublishScope {
    pub fn as_skill_scope(self) -> SkillScope {
        match self {
            Self::Project => SkillScope::Repo,
            Self::User => SkillScope::User,
        }
    }

    pub fn parse(raw: &str) -> Result<Self, PublishError> {
        match raw {
            "project" | "repo" | "local" => Ok(Self::Project),
            "user" => Ok(Self::User),
            _ => Err(PublishError::InvalidScope),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishError {
    InvalidScope,
    GenerationMismatch { expected: u64, actual: u64 },
    NotADirectory,
    SymlinkRejected,
    PathEscape,
    TooManyFiles,
    FileTooLarge,
    TreeTooLarge,
    UnexpectedFile,
    Quarantined,
    Evals(String),
    Io,
    Staging,
    AtomicSwap,
}

impl PublishError {
    pub fn as_code(&self) -> &'static str {
        match self {
            Self::InvalidScope => "invalid-scope",
            Self::GenerationMismatch { .. } => "generation-mismatch",
            Self::NotADirectory => "not-a-directory",
            Self::SymlinkRejected => "symlink-rejected",
            Self::PathEscape => "path-escape",
            Self::TooManyFiles => "too-many-files",
            Self::FileTooLarge => "file-too-large",
            Self::TreeTooLarge => "tree-too-large",
            Self::UnexpectedFile => "unexpected-file",
            Self::Quarantined => "quarantined",
            Self::Evals(_) => "evals-invalid",
            Self::Io => "io",
            Self::Staging => "staging",
            Self::AtomicSwap => "atomic-swap",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::InvalidScope => "Publish scope must be project or user.".into(),
            Self::GenerationMismatch { .. } => {
                "Publish generation does not match the current inventory generation.".into()
            }
            Self::NotADirectory => "Skill source is not a directory.".into(),
            Self::SymlinkRejected => "Skill trees must not contain symlinks.".into(),
            Self::PathEscape => "Skill tree contains a path that escapes the skill directory.".into(),
            Self::TooManyFiles => "Skill tree exceeds the 64-file bound.".into(),
            Self::FileTooLarge => "A skill file exceeds the 256 KiB bound.".into(),
            Self::TreeTooLarge => "Skill tree exceeds the 1 MiB bound.".into(),
            Self::UnexpectedFile => {
                "Skill tree contains a file outside SKILL.md, evals/, scripts/, references/, LICENSE, README.md, or NOTICE.".into()
            }
            Self::Quarantined => "Skill failed strict validation and was not published.".into(),
            Self::Evals(msg) => msg.clone(),
            Self::Io => "Skill publish could not read or write files.".into(),
            Self::Staging => "Skill staging directory could not be created.".into(),
            Self::AtomicSwap => "Skill publish could not atomically replace the destination.".into(),
        }
    }
}

impl From<EvalSchemaError> for PublishError {
    fn from(err: EvalSchemaError) -> Self {
        Self::Evals(err.message)
    }
}

impl std::fmt::Display for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishResult {
    pub identity: SkillIdentity,
    pub generation: u64,
    pub status: SkillHealthStatus,
    pub created: bool,
}

#[derive(Debug, Clone)]
struct TreeFile {
    relative: PathBuf,
    bytes: Vec<u8>,
}

/// Validate a complete skill directory without following symlinks.
pub fn validate_complete_skill_directory(
    source: &Path,
    scope: Option<SkillScope>,
) -> Result<SkillIdentity, PublishError> {
    let meta = fs::symlink_metadata(source).map_err(|_| PublishError::NotADirectory)?;
    if meta.file_type().is_symlink() {
        return Err(PublishError::SymlinkRejected);
    }
    if !meta.is_dir() {
        return Err(PublishError::NotADirectory);
    }
    match validate_strict_skill_dir(source, scope) {
        StrictSkillOutcome::Valid(discovered) => {
            collect_tree(source)?;
            if let Err(err) = load_eval_suite_from_dir(source) {
                return Err(PublishError::from(err));
            }
            Ok(discovered.identity)
        }
        StrictSkillOutcome::Quarantined(_) => Err(PublishError::Quarantined),
    }
}

/// Atomically publish `source` into `dest_parent/<name>/`.
///
/// `dest_parent` must already be an allowed skills root (caller-checked).
/// Staging is created as an owner-only sibling of the destination.
pub fn publish_skill_directory(
    source: &Path,
    dest_parent: &Path,
    scope: PublishScope,
    expected_generation: Option<u64>,
    current_generation: u64,
) -> Result<PublishResult, PublishError> {
    if let Some(expected) = expected_generation
        && expected != current_generation
    {
        return Err(PublishError::GenerationMismatch {
            expected,
            actual: current_generation,
        });
    }
    let identity = validate_complete_skill_directory(source, Some(scope.as_skill_scope()))?;
    let files = collect_tree(source)?;
    if identity.parent_dir_name.is_empty() {
        return Err(PublishError::NotADirectory);
    }
    let _guard = publish_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    ensure_real_collection_dir(dest_parent, scope)?;
    set_owner_only_dir(dest_parent);
    let dest = dest_parent.join(&identity.parent_dir_name);
    let created = match fs::symlink_metadata(&dest) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(PublishError::SymlinkRejected);
        }
        Ok(_) => false,
        Err(_) => true,
    };
    let staging_root = dest_parent.join(STAGING_DIR_NAME);
    mkdir_real(&staging_root)?;
    let nonce = format!(
        "tmp-{}-{}",
        std::process::id(),
        STAGING_SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let staging_holder = staging_root.join(&nonce);
    if let Err(err) = mkdir_real(&staging_holder) {
        remove_verified_staging_holder(dest_parent, &staging_holder);
        return Err(err);
    }
    // The skill directory name must match `name` for strict validation.
    let staging = staging_holder.join(&identity.parent_dir_name);
    if let Err(err) = mkdir_real(&staging) {
        remove_verified_staging_holder(dest_parent, &staging_holder);
        return Err(err);
    }

    if let Err(err) = materialize_tree(&staging, &files) {
        remove_verified_staging_holder(dest_parent, &staging_holder);
        return Err(err);
    }
    if let Err(err) = validate_complete_skill_directory(&staging, Some(scope.as_skill_scope())) {
        remove_verified_staging_holder(dest_parent, &staging_holder);
        return Err(err);
    }

    if let Err(err) = atomic_replace(&staging, &dest) {
        remove_verified_staging_holder(dest_parent, &staging_holder);
        return Err(err);
    }
    remove_verified_staging_holder(dest_parent, &staging_holder);
    Ok(PublishResult {
        identity,
        generation: current_generation.saturating_add(1),
        status: SkillHealthStatus::Untested,
        created,
    })
}

/// Resolve the destination parent for a publish scope.
pub fn dest_parent_for_scope(
    scope: PublishScope,
    cwd: &Path,
    grok_home: &Path,
) -> Result<PathBuf, PublishError> {
    match scope {
        PublishScope::User => Ok(grok_home.join("skills")),
        PublishScope::Project => Ok(project_root(cwd).join(".grok").join("skills")),
    }
}

/// Create `dest_parent` as a real directory. Reject a symlink collection or a
/// project-scope symlink `.grok` parent so rename cannot land outside the
/// allowed root. `$GROK_HOME` may itself be a symlink for user-scope skills/.
fn ensure_real_collection_dir(dest_parent: &Path, scope: PublishScope) -> Result<(), PublishError> {
    reject_project_grok_parent_symlink(dest_parent, scope)?;
    match fs::symlink_metadata(dest_parent) {
        Ok(meta) if meta.file_type().is_symlink() => Err(PublishError::SymlinkRejected),
        Ok(meta) if meta.is_dir() => verify_collection_stays_under_parent(dest_parent),
        Ok(_) => Err(PublishError::Staging),
        Err(_) => {
            let parent = dest_parent.parent().ok_or(PublishError::PathEscape)?;
            match fs::symlink_metadata(parent) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    if scope == PublishScope::User {
                        mkdir_real(dest_parent)?;
                        verify_collection_stays_under_parent(dest_parent)
                    } else {
                        Err(PublishError::SymlinkRejected)
                    }
                }
                Ok(meta) if meta.is_dir() => {
                    mkdir_real(dest_parent)?;
                    verify_collection_stays_under_parent(dest_parent)
                }
                Ok(_) => Err(PublishError::Staging),
                Err(_) => {
                    if dest_parent.file_name().is_some_and(|n| n == "skills")
                        && parent.file_name().is_some_and(|n| n == ".grok")
                    {
                        ensure_real_collection_dir(parent, scope)?;
                        mkdir_real(dest_parent)?;
                        verify_collection_stays_under_parent(dest_parent)
                    } else {
                        Err(PublishError::Staging)
                    }
                }
            }
        }
    }
}

/// Always lstat `dest_parent.parent()`. A project `.grok` that is a symlink
/// must be rejected even when `skills/` already exists as a real directory
/// under the symlink target. User-scope `$GROK_HOME` may be a symlink.
fn reject_project_grok_parent_symlink(
    dest_parent: &Path,
    scope: PublishScope,
) -> Result<(), PublishError> {
    let Some(parent) = dest_parent.parent() else {
        return Err(PublishError::PathEscape);
    };
    match fs::symlink_metadata(parent) {
        Ok(meta)
            if meta.file_type().is_symlink()
                && scope == PublishScope::Project
                && parent.file_name().is_some_and(|n| n == ".grok") =>
        {
            Err(PublishError::SymlinkRejected)
        }
        _ => Ok(()),
    }
}

fn mkdir_real(path: &Path) -> Result<(), PublishError> {
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(_) => match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(PublishError::SymlinkRejected),
            Ok(meta) if meta.is_dir() => {}
            _ => return Err(PublishError::Staging),
        },
    }
    set_owner_only_dir(path);
    let meta = fs::symlink_metadata(path).map_err(|_| PublishError::Staging)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(PublishError::SymlinkRejected);
    }
    Ok(())
}

/// Remove a staging holder only when it is a real directory under
/// `dest_parent/.staging`. Never follow a symlink out of the skills root.
fn remove_verified_staging_holder(dest_parent: &Path, staging_holder: &Path) {
    let staging_root = dest_parent.join(STAGING_DIR_NAME);
    match fs::symlink_metadata(&staging_root) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => return,
        Ok(_) => {}
        Err(_) => return,
    }
    match fs::symlink_metadata(staging_holder) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => return,
        Ok(_) => {}
        Err(_) => return,
    }
    let Ok(parent_real) = fs::canonicalize(dest_parent) else {
        return;
    };
    let Ok(root_real) = fs::canonicalize(&staging_root) else {
        return;
    };
    let Ok(holder_real) = fs::canonicalize(staging_holder) else {
        return;
    };
    if !root_real.starts_with(&parent_real) || !holder_real.starts_with(&root_real) {
        return;
    }
    let _ = fs::remove_dir_all(&holder_real);
}

fn verify_collection_stays_under_parent(dest_parent: &Path) -> Result<(), PublishError> {
    let Some(parent) = dest_parent.parent() else {
        return Err(PublishError::PathEscape);
    };
    let dest_real = fs::canonicalize(dest_parent).map_err(|_| PublishError::Staging)?;
    let parent_real = fs::canonicalize(parent).map_err(|_| PublishError::PathEscape)?;
    if !dest_real.starts_with(&parent_real) {
        return Err(PublishError::PathEscape);
    }
    Ok(())
}

fn project_root(cwd: &Path) -> PathBuf {
    let mut current = cwd.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return current;
        }
        if !current.pop() {
            return cwd.to_path_buf();
        }
    }
}

fn collect_tree(source: &Path) -> Result<Vec<TreeFile>, PublishError> {
    let mut files = Vec::new();
    let mut total = 0u64;
    collect_tree_inner(source, source, &mut files, &mut total)?;
    if files
        .iter()
        .all(|f| f.relative != Path::new(SKILL_MD_FILE_NAME))
    {
        return Err(PublishError::Quarantined);
    }
    Ok(files)
}

fn collect_tree_inner(
    source: &Path,
    dir: &Path,
    files: &mut Vec<TreeFile>,
    total: &mut u64,
) -> Result<(), PublishError> {
    let entries = fs::read_dir(dir).map_err(|_| PublishError::Io)?;
    for entry in entries {
        let entry = entry.map_err(|_| PublishError::Io)?;
        let path = entry.path();
        let name = entry.file_name();
        if name == STAGING_DIR_NAME {
            continue;
        }
        let meta = fs::symlink_metadata(&path).map_err(|_| PublishError::Io)?;
        if meta.file_type().is_symlink() {
            return Err(PublishError::SymlinkRejected);
        }
        let relative = path
            .strip_prefix(source)
            .map_err(|_| PublishError::PathEscape)?;
        if relative
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
        {
            return Err(PublishError::PathEscape);
        }
        if meta.is_dir() {
            if !allowed_dir(relative) {
                return Err(PublishError::UnexpectedFile);
            }
            collect_tree_inner(source, &path, files, total)?;
            continue;
        }
        if !meta.is_file() {
            return Err(PublishError::UnexpectedFile);
        }
        if !allowed_file(relative) {
            return Err(PublishError::UnexpectedFile);
        }
        if files.len() >= MAX_SKILL_TREE_FILES {
            return Err(PublishError::TooManyFiles);
        }
        if meta.len() > MAX_SKILL_FILE_BYTES {
            return Err(PublishError::FileTooLarge);
        }
        *total = total.saturating_add(meta.len());
        if *total > MAX_SKILL_TREE_BYTES {
            return Err(PublishError::TreeTooLarge);
        }
        let bytes = fs::read(&path).map_err(|_| PublishError::Io)?;
        files.push(TreeFile {
            relative: relative.to_path_buf(),
            bytes,
        });
    }
    Ok(())
}

fn allowed_dir(relative: &Path) -> bool {
    match relative
        .components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
    {
        Some("evals" | "scripts" | "references") => relative.components().count() <= 4,
        _ => false,
    }
}

fn allowed_file(relative: &Path) -> bool {
    if relative == Path::new(SKILL_MD_FILE_NAME)
        || relative == Path::new("LICENSE")
        || relative == Path::new("README.md")
        || relative == Path::new("NOTICE")
        || relative == Path::new("evals").join("cases.yaml")
    {
        return true;
    }
    match relative
        .components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
    {
        Some("scripts" | "references") => {
            relative.extension().is_some() || relative.components().count() >= 2
        }
        _ => false,
    }
}

fn materialize_tree(staging: &Path, files: &[TreeFile]) -> Result<(), PublishError> {
    for file in files {
        let dest = staging.join(&file.relative);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|_| PublishError::Staging)?;
            set_owner_only_dir(parent);
        }
        atomic_write(&dest, &file.bytes)?;
    }
    set_owner_only_dir(staging);
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), PublishError> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|_| PublishError::Staging)?;
    set_owner_only_file(&tmp);
    fs::rename(&tmp, path).map_err(|_| PublishError::Staging)?;
    Ok(())
}

fn atomic_replace(staging: &Path, dest: &Path) -> Result<(), PublishError> {
    let staging_meta = fs::symlink_metadata(staging).map_err(|_| PublishError::AtomicSwap)?;
    if staging_meta.file_type().is_symlink() || !staging_meta.is_dir() {
        return Err(PublishError::AtomicSwap);
    }
    if !dest.exists() {
        return fs::rename(staging, dest).map_err(|_| PublishError::AtomicSwap);
    }
    let backup = dest.with_extension("bak-publish");
    let _ = fs::remove_dir_all(&backup);
    fs::rename(dest, &backup).map_err(|_| PublishError::AtomicSwap)?;
    match fs::rename(staging, dest) {
        Ok(()) => {
            let _ = fs::remove_dir_all(&backup);
            Ok(())
        }
        Err(_) => {
            let _ = fs::rename(&backup, dest);
            Err(PublishError::AtomicSwap)
        }
    }
}

fn set_owner_only_dir(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
    }
    let _ = path;
}

fn set_owner_only_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    let _ = path;
}

/// Render a SKILL.md from wizard/tool fields. Callers must pass already-bounded
/// official fields; no repair is applied.
pub fn render_skill_md(name: &str, description: &str, body: &str) -> String {
    let mut out = String::from("---\n");
    out.push_str("name: ");
    out.push_str(name);
    out.push('\n');
    out.push_str("description: ");
    out.push_str(&yaml_quote(description));
    out.push('\n');
    out.push_str("---\n\n");
    if body.trim().is_empty() {
        out.push_str("# ");
        out.push_str(name);
        out.push('\n');
    } else {
        out.push_str(body.trim_end());
        out.push('\n');
    }
    out
}

fn yaml_quote(value: &str) -> String {
    if value.is_empty()
        || value.contains(':')
        || value.contains('#')
        || value.contains('\n')
        || value.starts_with([' ', '*', '&', '!', '|', '>', '%', '@', '`'])
    {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(dir: &Path, name: &str, description: &str) {
        fs::create_dir_all(dir).unwrap();
        let body = format!(
            "---\nname: {name}\ndescription: \"{description}\"\n---\n\n# {name}\n\nDo the work.\n"
        );
        fs::write(dir.join("SKILL.md"), body).unwrap();
        if let crate::implementations::skills::strict::StrictSkillOutcome::Quarantined(row) =
            crate::implementations::skills::strict::validate_strict_skill_dir(dir, None)
        {
            panic!(
                "fixture quarantined: {:?}",
                row.diagnostics
                    .iter()
                    .map(|d| d.code.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn atomic_publish_creates_complete_skill() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src/commit");
        write_skill(&source, "commit", "Create well-formatted git commits.");
        let dest_parent = tmp.path().join("dest");
        let result =
            publish_skill_directory(&source, &dest_parent, PublishScope::User, Some(3), 3).unwrap();
        assert!(result.created);
        assert_eq!(result.identity.parent_dir_name, "commit");
        assert_eq!(result.generation, 4);
        assert!(dest_parent.join("commit/SKILL.md").is_file());
        assert!(!dest_parent.join(".staging/commit-").exists());
        let leftover = fs::read_dir(dest_parent.join(STAGING_DIR_NAME))
            .map(|d| d.count())
            .unwrap_or(0);
        assert_eq!(leftover, 0, "staging must not leave a partial active skill");
    }

    #[test]
    fn generation_mismatch_does_not_write() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src/commit");
        write_skill(&source, "commit", "Create well-formatted git commits.");
        let dest_parent = tmp.path().join("dest");
        let err = publish_skill_directory(&source, &dest_parent, PublishScope::User, Some(1), 2)
            .unwrap_err();
        assert!(matches!(
            err,
            PublishError::GenerationMismatch {
                expected: 1,
                actual: 2
            }
        ));
        assert!(!dest_parent.join("commit").exists());
    }

    #[test]
    fn symlink_in_tree_is_rejected_and_dest_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src/commit");
        write_skill(&source, "commit", "Create well-formatted git commits.");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", source.join("secrets")).unwrap();
            let dest_parent = tmp.path().join("dest");
            fs::create_dir_all(dest_parent.join("commit")).unwrap();
            fs::write(dest_parent.join("commit/SKILL.md"), "keep").unwrap();
            let err = publish_skill_directory(&source, &dest_parent, PublishScope::User, None, 0)
                .unwrap_err();
            assert_eq!(err, PublishError::SymlinkRejected);
            assert_eq!(
                fs::read_to_string(dest_parent.join("commit/SKILL.md")).unwrap(),
                "keep"
            );
        }
    }

    #[test]
    fn quarantined_source_is_not_published() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src/bad");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "no frontmatter\n").unwrap();
        let dest_parent = tmp.path().join("dest");
        let err = publish_skill_directory(&source, &dest_parent, PublishScope::Project, None, 0)
            .unwrap_err();
        assert_eq!(err, PublishError::Quarantined);
        assert!(!dest_parent.join("bad").exists());
    }

    #[test]
    fn unexpected_file_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src/commit");
        write_skill(&source, "commit", "Create well-formatted git commits.");
        fs::write(source.join("payload.bin"), [0u8; 8]).unwrap();
        let err = validate_complete_skill_directory(&source, None).unwrap_err();
        assert_eq!(err, PublishError::UnexpectedFile);
    }

    #[test]
    fn replace_rejects_non_directory_staging_and_keeps_active() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("commit");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("SKILL.md"), "active").unwrap();
        let staging = tmp.path().join("stage");
        fs::write(&staging, "not-a-dir").unwrap();
        let err = atomic_replace(&staging, &dest).unwrap_err();
        assert_eq!(err, PublishError::AtomicSwap);
        assert_eq!(fs::read_to_string(dest.join("SKILL.md")).unwrap(), "active");
    }

    #[cfg(unix)]
    #[test]
    fn staging_root_symlink_is_rejected_and_target_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src/commit");
        write_skill(&source, "commit", "Create well-formatted git commits.");
        let dest_parent = tmp.path().join("dest");
        fs::create_dir_all(dest_parent.join("commit")).unwrap();
        fs::write(dest_parent.join("commit/SKILL.md"), "keep").unwrap();
        let outside = tmp.path().join("outside-staging");
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, dest_parent.join(STAGING_DIR_NAME)).unwrap();
        let err = publish_skill_directory(&source, &dest_parent, PublishScope::User, None, 0)
            .unwrap_err();
        assert_eq!(err, PublishError::SymlinkRejected);
        assert_eq!(
            fs::read_to_string(dest_parent.join("commit/SKILL.md")).unwrap(),
            "keep"
        );
        let leaked = fs::read_dir(&outside)
            .unwrap()
            .any(|entry| entry.unwrap().path().join("SKILL.md").is_file());
        assert!(
            !leaked,
            "materialize must not write SKILL.md through dest/.staging symlink"
        );
        assert!(
            !outside.join("commit/SKILL.md").exists(),
            "active skill name must not appear under the symlink target"
        );
    }

    #[cfg(unix)]
    #[test]
    fn dest_parent_symlink_is_rejected_and_target_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src/commit");
        write_skill(&source, "commit", "Create well-formatted git commits.");
        let target = tmp.path().join("outside");
        fs::create_dir_all(&target).unwrap();
        let dest_parent = tmp.path().join("skills");
        std::os::unix::fs::symlink(&target, &dest_parent).unwrap();
        let err = publish_skill_directory(&source, &dest_parent, PublishScope::User, None, 0)
            .unwrap_err();
        assert_eq!(err, PublishError::SymlinkRejected);
        assert!(
            !target.join("commit").exists(),
            "atomic rename must not install outside the allowed root"
        );
    }

    #[cfg(unix)]
    #[test]
    fn project_grok_parent_symlink_is_rejected_when_skills_dir_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src/commit");
        write_skill(&source, "commit", "Create well-formatted git commits.");
        let project = tmp.path().join("project");
        fs::create_dir_all(project.join(".git")).unwrap();
        let outside = tmp.path().join("outside");
        fs::create_dir_all(outside.join("skills")).unwrap();
        fs::write(outside.join("skills/sentinel"), "keep").unwrap();
        std::os::unix::fs::symlink(&outside, project.join(".grok")).unwrap();
        let dest_parent = project.join(".grok").join("skills");
        assert!(
            dest_parent.is_dir(),
            "existing real skills/ under the symlink target is the dangerous case"
        );
        let err = publish_skill_directory(&source, &dest_parent, PublishScope::Project, None, 0)
            .unwrap_err();
        assert_eq!(err, PublishError::SymlinkRejected);
        assert!(
            !outside.join("skills/commit").exists(),
            "publish must not install into the symlink target"
        );
        assert_eq!(
            fs::read_to_string(outside.join("skills/sentinel")).unwrap(),
            "keep"
        );
        let leftover = fs::read_dir(outside.join("skills"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(leftover.len(), 1, "tmp/outside/skills must be untouched");
        assert_eq!(leftover[0], std::ffi::OsString::from("sentinel"));
    }

    #[cfg(unix)]
    #[test]
    fn user_scope_allows_grok_home_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src/commit");
        write_skill(&source, "commit", "Create well-formatted git commits.");
        let real_home = tmp.path().join("real-home");
        fs::create_dir_all(&real_home).unwrap();
        let grok_home = tmp.path().join(".grok");
        std::os::unix::fs::symlink(&real_home, &grok_home).unwrap();
        let dest_parent = grok_home.join("skills");
        let result =
            publish_skill_directory(&source, &dest_parent, PublishScope::User, None, 0).unwrap();
        assert!(result.created);
        assert!(real_home.join("skills/commit/SKILL.md").is_file());
        assert!(
            fs::symlink_metadata(&grok_home)
                .unwrap()
                .file_type()
                .is_symlink(),
            "user-scope must not require GROK_HOME to be a real directory"
        );
    }

    #[test]
    fn overlapping_publishes_leave_complete_trees() {
        let tmp = tempfile::tempdir().unwrap();
        let dest_parent = tmp.path().join("dest");
        fs::create_dir_all(&dest_parent).unwrap();
        let src_a = tmp.path().join("src/alpha");
        let src_b = tmp.path().join("src/beta");
        write_skill(&src_a, "alpha", "Create well-formatted git commits.");
        write_skill(&src_b, "beta", "Create well-formatted git commits.");
        std::thread::scope(|s| {
            let dest = dest_parent.as_path();
            s.spawn(|| {
                publish_skill_directory(&src_a, dest, PublishScope::User, None, 0).unwrap();
            });
            s.spawn(|| {
                publish_skill_directory(&src_b, dest, PublishScope::User, None, 0).unwrap();
            });
        });
        let alpha = fs::read_to_string(dest_parent.join("alpha/SKILL.md")).unwrap();
        let beta = fs::read_to_string(dest_parent.join("beta/SKILL.md")).unwrap();
        assert!(alpha.contains("name: alpha"), "{alpha}");
        assert!(beta.contains("name: beta"), "{beta}");
        assert!(!alpha.contains("name: beta"));
        assert!(!beta.contains("name: alpha"));
        assert!(dest_parent.join("alpha/SKILL.md").is_file());
        assert!(dest_parent.join("beta/SKILL.md").is_file());
    }
}
