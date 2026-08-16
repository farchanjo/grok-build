//! Companion `model_route.json` + private `model_identity.meta` pair.
//!
//! Route provenance is **not** a public `Summary` field. It is stored as a
//! secret-free companion next to `summary.json`, bound via private meta
//! (pair_id + summary digest). Leave never adopts a mismatched companion.
//!
//! Identity I/O is a **multi-component trusted-root walk**: starting from the
//! filesystem root, each path component of the session directory is opened
//! with `openat` + `O_NOFOLLOW` + `O_CLOEXEC` + `O_DIRECTORY`, owner/mode
//! checked, then final basenames (summary/companion/meta) are operated only
//! through that session dirfd. Intermediate ancestor symlinks (e.g. a planted
//! `sessions` → `/evil/sessions`) fail closed. No check-then-path use on
//! production identity artifacts.
//!
//! Transaction journal: `model_identity.txn` records staged temp names and
//! digests. Recovery rolls forward a complete staged set or rolls back temps.
//! Ordinary summary patches (chat/title/git/activity) use Leave to keep the
//! companion digest aligned with the rewritten summary.

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
pub(crate) const SUMMARY_FILE: &str = "summary.json";

const MAX_META_BYTES: usize = 4096;
const META_VERSION: u32 = 1;
const TXN_VERSION: u32 = 1;

const SUMMARY_TMP: &str = "summary.json.identity.tmp";
const COMPANION_TMP: &str = "model_route.json.identity.tmp";
const META_TMP: &str = "model_identity.meta.identity.tmp";

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
    /// Staged temp basenames under the session root (never absolute paths).
    summary_tmp: Option<String>,
    companion_tmp: Option<String>,
    meta_tmp: Option<String>,
    new_summary_sha: Option<String>,
    new_companion_sha: Option<String>,
    new_meta_sha: Option<String>,
    previous_summary_sha: Option<String>,
    /// When true, recovery may rename staged temps into finals (complete set).
    ready_to_commit: bool,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn validate_single_component(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
        || Path::new(name).components().count() != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "identity basenames must be single non-traversing components",
        ));
    }
    Ok(())
}

// ── Trusted-root dirfd containment (unix) ───────────────────────────────────

#[cfg(unix)]
mod contain {
    use super::*;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::path::Component;

    pub struct SessionRoot {
        dir: File,
        path: PathBuf,
    }

    impl SessionRoot {
        /// Multi-component trusted-root walk.
        ///
        /// Production layout is `{trusted}/sessions/{cwd}/{id}`. The trusted
        /// anchor (parent of `sessions`, or parent of the session dir for
        /// explicit/subagent paths) is path-opened once as policy-trusted, then
        /// every subsequent component is `openat`+`O_NOFOLLOW`+`O_DIRECTORY`
        /// with owner/mode checks. An intermediate `sessions` symlink fails
        /// closed (ELOOP).
        pub fn open(session_dir: &Path) -> io::Result<Self> {
            let path = if session_dir.is_absolute() {
                session_dir.to_path_buf()
            } else {
                std::env::current_dir()?.join(session_dir)
            };
            let (trusted, relative) = split_trusted_and_relative(&path)?;
            open_under(trusted.as_path(), relative.as_path(), path)
        }

        pub fn path(&self) -> &Path {
            &self.path
        }

        fn openat(&self, name: &str, flags: i32, mode: u32) -> io::Result<File> {
            validate_single_component(name)?;
            let os = std::ffi::OsStr::new(name);
            openat_component(&self.dir, os, flags, mode)
        }

        pub fn exists_nofollow(&self, name: &str) -> io::Result<bool> {
            match self.openat(
                name,
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
                0,
            ) {
                Ok(_) => Ok(true),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
                // ELOOP: final component is a symlink — treat as present but unusable.
                Err(e) if e.raw_os_error() == Some(libc::ELOOP) => Ok(true),
                Err(e) => Err(e),
            }
        }

        pub fn read_regular(&self, name: &str) -> io::Result<Vec<u8>> {
            let mut f = self.openat(
                name,
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
                0,
            )?;
            owner_mode_check_file(&f)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            Ok(buf)
        }

        pub fn write_staged(&self, tmp_name: &str, bytes: &[u8]) -> io::Result<()> {
            // Exclusive create — refuse if a symlink or existing file is present.
            let mut f = self.openat(
                tmp_name,
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )?;
            f.write_all(bytes)?;
            f.sync_all()?;
            // Ensure mode is 0600 even if umask interfered.
            let _ = f.set_permissions(fs::Permissions::from_mode(0o600));
            Ok(())
        }

        pub fn rename_nofollow(&self, from: &str, to: &str) -> io::Result<()> {
            validate_single_component(from)?;
            validate_single_component(to)?;
            let c_from = std::ffi::CString::new(from.as_bytes()).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "identity name contains NUL")
            })?;
            let c_to = std::ffi::CString::new(to.as_bytes()).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "identity name contains NUL")
            })?;
            // SAFETY: both names are single components relative to the same dirfd.
            let rc = unsafe {
                libc::renameat(
                    self.dir.as_raw_fd(),
                    c_from.as_ptr(),
                    self.dir.as_raw_fd(),
                    c_to.as_ptr(),
                )
            };
            if rc != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        pub fn unlink_nofollow(&self, name: &str) -> io::Result<()> {
            validate_single_component(name)?;
            let c_name = std::ffi::CString::new(name.as_bytes()).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "identity name contains NUL")
            })?;
            // SAFETY: single component relative to session dirfd; O_NOFOLLOW via unlinkat
            // is implicit for non-directory (AT_REMOVEDIR not set). Symlink final
            // components are unlinked themselves (correct: refuse to follow).
            let rc = unsafe { libc::unlinkat(self.dir.as_raw_fd(), c_name.as_ptr(), 0) };
            if rc != 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::NotFound {
                    return Ok(());
                }
                return Err(err);
            }
            Ok(())
        }

        pub fn lock_exclusive(&self) -> io::Result<File> {
            // Create lock via openat so a swapped parent cannot redirect us.
            let f = match self.openat(
                MODEL_IDENTITY_LOCK,
                libc::O_RDWR | libc::O_CREAT | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            ) {
                Ok(f) => f,
                Err(e) if e.raw_os_error() == Some(libc::ELOOP) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "refusing symlink model_identity.lock",
                    ));
                }
                Err(e) => return Err(e),
            };
            use fs2::FileExt;
            f.lock_exclusive()?;
            Ok(f)
        }

        /// Re-open the session path and confirm it is still the same directory
        /// inode (TOCTOU parent-swap detection after staging).
        pub fn revalidate(&self) -> io::Result<()> {
            let again = SessionRoot::open(&self.path)?;
            let a = self.dir.metadata()?;
            let b = again.dir.metadata()?;
            use std::os::unix::fs::MetadataExt;
            if a.dev() != b.dev() || a.ino() != b.ino() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "session root replaced between identity open and commit (TOCTOU)",
                ));
            }
            Ok(())
        }

        pub fn fsync_dir(&self) -> io::Result<()> {
            self.dir.sync_all()
        }
    }

    /// Split `{…}/sessions/{cwd}/{id}` into trusted parent of `sessions` and
    /// relative `sessions/cwd/id`. Otherwise fall back to parent + basename.
    fn split_trusted_and_relative(path: &Path) -> io::Result<(PathBuf, PathBuf)> {
        let comps: Vec<Component<'_>> = path.components().collect();
        let sessions_idx = comps.iter().position(
            |c| matches!(c, Component::Normal(n) if *n == std::ffi::OsStr::new("sessions")),
        );
        if let Some(idx) = sessions_idx {
            if idx == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "sessions cannot be the path root for identity walk",
                ));
            }
            // Rebuild trusted prefix (includes RootDir) and relative suffix.
            let trusted = comps[..idx].iter().collect::<PathBuf>();
            let relative = comps[idx..].iter().collect::<PathBuf>();
            if relative.as_os_str().is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "empty relative identity path",
                ));
            }
            return Ok((trusted, relative));
        }
        let parent = path.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "session path has no trusted parent",
            )
        })?;
        let name = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "session path has no basename")
        })?;
        Ok((parent.to_path_buf(), PathBuf::from(name)))
    }

    fn open_under(trusted: &Path, relative: &Path, final_path: PathBuf) -> io::Result<SessionRoot> {
        // Policy-trusted anchor: path open with O_DIRECTORY|O_CLOEXEC. Prefer
        // O_NOFOLLOW so the anchor itself cannot be a final-component symlink.
        let mut current = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(trusted)
            .map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!("open trusted identity anchor {}: {e}", trusted.display()),
                )
            })?;
        owner_mode_check_dir(&current)?;

        for component in relative.components() {
            match component {
                Component::Normal(name) => {
                    current = openat_component(
                        &current,
                        name,
                        libc::O_RDONLY
                            | libc::O_DIRECTORY
                            | libc::O_CLOEXEC
                            | libc::O_NOFOLLOW
                            | libc::O_NONBLOCK,
                        0,
                    )
                    .map_err(|e| {
                        io::Error::new(
                            e.kind(),
                            format!(
                                "openat component {} under {}: {e}",
                                name.to_string_lossy(),
                                final_path.display()
                            ),
                        )
                    })?;
                    owner_mode_check_dir(&current)?;
                }
                Component::CurDir => {}
                Component::RootDir | Component::Prefix(_) | Component::ParentDir => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "identity relative walk refuses non-normal components",
                    ));
                }
            }
        }
        Ok(SessionRoot {
            dir: current,
            path: final_path,
        })
    }

    fn openat_component(
        directory: &File,
        name: &std::ffi::OsStr,
        flags: i32,
        mode: u32,
    ) -> io::Result<File> {
        let c_name = std::ffi::CString::new(name.as_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "identity name contains NUL")
        })?;
        // SAFETY: directory fd is live; name is NUL-terminated single component.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), c_name.as_ptr(), flags, mode) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: nonnegative openat result transfers one owned fd.
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    fn owner_mode_check_dir(dir: &File) -> io::Result<()> {
        use std::os::unix::fs::MetadataExt;
        let meta = dir.metadata()?;
        if !meta.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "identity walk component is not a directory",
            ));
        }
        let uid = unsafe { libc::getuid() };
        if meta.uid() != uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "identity walk component not owned by current user",
            ));
        }
        // Refuse world-writable dirs (symlink-plant surface).
        if meta.mode() & 0o002 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "identity walk component is world-writable",
            ));
        }
        Ok(())
    }

    fn owner_mode_check_file(file: &File) -> io::Result<()> {
        use std::os::unix::fs::MetadataExt;
        let meta = file.metadata()?;
        if !meta.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "identity path is not a regular file",
            ));
        }
        let uid = unsafe { libc::getuid() };
        if meta.uid() != uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "identity file not owned by current user",
            ));
        }
        Ok(())
    }

    /// Test helper: expose raw dirfd open for parent-swap harnesses.
    #[cfg(test)]
    pub fn open_root_for_test(session_dir: &Path) -> io::Result<SessionRoot> {
        SessionRoot::open(session_dir)
    }
}

#[cfg(not(unix))]
mod contain {
    use super::*;

    pub struct SessionRoot {
        path: PathBuf,
    }

    impl SessionRoot {
        pub fn open(session_dir: &Path) -> io::Result<Self> {
            if !session_dir.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "session root missing",
                ));
            }
            Ok(Self {
                path: session_dir.to_path_buf(),
            })
        }

        pub fn path(&self) -> &Path {
            &self.path
        }

        pub fn exists_nofollow(&self, name: &str) -> io::Result<bool> {
            validate_single_component(name)?;
            Ok(self.path.join(name).exists())
        }

        pub fn read_regular(&self, name: &str) -> io::Result<Vec<u8>> {
            validate_single_component(name)?;
            fs::read(self.path.join(name))
        }

        pub fn write_staged(&self, tmp_name: &str, bytes: &[u8]) -> io::Result<()> {
            validate_single_component(tmp_name)?;
            let p = self.path.join(tmp_name);
            let mut f = OpenOptions::new().write(true).create_new(true).open(&p)?;
            f.write_all(bytes)?;
            f.sync_all()?;
            Ok(())
        }

        pub fn rename_nofollow(&self, from: &str, to: &str) -> io::Result<()> {
            validate_single_component(from)?;
            validate_single_component(to)?;
            fs::rename(self.path.join(from), self.path.join(to))
        }

        pub fn unlink_nofollow(&self, name: &str) -> io::Result<()> {
            validate_single_component(name)?;
            match fs::remove_file(self.path.join(name)) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            }
        }

        pub fn lock_exclusive(&self) -> io::Result<File> {
            let f = OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .open(self.path.join(MODEL_IDENTITY_LOCK))?;
            use fs2::FileExt;
            f.lock_exclusive()?;
            Ok(f)
        }

        pub fn revalidate(&self) -> io::Result<()> {
            Ok(())
        }

        pub fn fsync_dir(&self) -> io::Result<()> {
            Ok(())
        }
    }
}

use contain::SessionRoot;

fn write_txn_marker(root: &SessionRoot, marker: &TxnMarker) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(marker).map_err(io::Error::other)?;
    // Stage txn as exclusive temp then rename so the marker itself is atomic.
    let tmp = "model_identity.txn.staging";
    let _ = root.unlink_nofollow(tmp);
    root.write_staged(tmp, &bytes)?;
    root.rename_nofollow(tmp, MODEL_IDENTITY_TXN)?;
    let _ = root.fsync_dir();
    Ok(())
}

fn clear_txn_marker(root: &SessionRoot) -> io::Result<()> {
    root.unlink_nofollow(MODEL_IDENTITY_TXN)?;
    let _ = root.fsync_dir();
    Ok(())
}

fn recover_identity_txn(root: &SessionRoot) -> io::Result<()> {
    if !root.exists_nofollow(MODEL_IDENTITY_TXN)? {
        // Best-effort cleanup of any orphaned staged temps.
        for name in [SUMMARY_TMP, COMPANION_TMP, META_TMP] {
            let _ = root.unlink_nofollow(name);
        }
        return Ok(());
    }
    let marker_bytes = match root.read_regular(MODEL_IDENTITY_TXN) {
        Ok(b) => b,
        Err(_) => {
            let _ = root.unlink_nofollow(MODEL_IDENTITY_TXN);
            return Ok(());
        }
    };
    let marker: TxnMarker = match serde_json::from_slice(&marker_bytes) {
        Ok(m) => m,
        Err(_) => {
            // Corrupt journal: drop marker + temps, fail closed to previous finals.
            for name in [SUMMARY_TMP, COMPANION_TMP, META_TMP] {
                let _ = root.unlink_nofollow(name);
            }
            let _ = root.unlink_nofollow(MODEL_IDENTITY_TXN);
            return Ok(());
        }
    };

    if marker.ready_to_commit {
        // Roll forward if staged digests match the journal.
        let artifact_matches = |tmp: &str, final_name: &str, expected: &str| {
            root.read_regular(tmp)
                .or_else(|_| root.read_regular(final_name))
                .is_ok_and(|bytes| sha256_hex(&bytes) == expected)
        };
        let summary_ok = match (&marker.summary_tmp, &marker.new_summary_sha) {
            (Some(tmp), Some(sha)) => artifact_matches(tmp, SUMMARY_FILE, sha),
            (None, None) => true,
            _ => false,
        };
        let companion_ok = match (&marker.companion_tmp, &marker.new_companion_sha) {
            (Some(tmp), Some(sha)) => artifact_matches(tmp, MODEL_ROUTE_FILE, sha),
            (None, None) => true,
            (None, Some(_)) | (Some(_), None) => false,
        };
        let meta_ok = match (&marker.meta_tmp, &marker.new_meta_sha) {
            (Some(tmp), Some(sha)) => artifact_matches(tmp, MODEL_IDENTITY_META, sha),
            (None, None) => true,
            _ => false,
        };

        if summary_ok && companion_ok && meta_ok {
            if let Some(tmp) = &marker.summary_tmp
                && root.exists_nofollow(tmp)?
            {
                root.rename_nofollow(tmp, SUMMARY_FILE)?;
            }
            if let Some(tmp) = &marker.companion_tmp {
                if root.exists_nofollow(tmp)? {
                    root.rename_nofollow(tmp, MODEL_ROUTE_FILE)?;
                }
            } else if marker.meta_tmp.is_none() {
                // A transaction staging neither identity artifact intentionally
                // commits a summary without provenance. Remove both old finals.
                root.unlink_nofollow(MODEL_ROUTE_FILE)?;
                root.unlink_nofollow(MODEL_IDENTITY_META)?;
            }
            if let Some(tmp) = &marker.meta_tmp
                && root.exists_nofollow(tmp)?
            {
                root.rename_nofollow(tmp, MODEL_IDENTITY_META)?;
            }
            clear_txn_marker(root)?;
            return Ok(());
        }
    }

    // Incomplete / mismatched staged set: roll back temps, keep previous finals.
    for name in [
        marker.summary_tmp.as_deref(),
        marker.companion_tmp.as_deref(),
        marker.meta_tmp.as_deref(),
        Some(SUMMARY_TMP),
        Some(COMPANION_TMP),
        Some(META_TMP),
    ]
    .into_iter()
    .flatten()
    {
        let _ = root.unlink_nofollow(name);
    }
    clear_txn_marker(root)?;
    Ok(())
}

/// Load companion provenance if present and bound to the summary.
/// Old sessions without companion/meta load successfully (no rewrite).
pub fn load_route_companion(
    session_dir: &Path,
    summary: &Summary,
) -> io::Result<Option<ModelRouteProvenance>> {
    let root = SessionRoot::open(session_dir)?;
    recover_identity_txn(&root)?;

    let companion_exists = root.exists_nofollow(MODEL_ROUTE_FILE)?;
    let meta_exists = root.exists_nofollow(MODEL_IDENTITY_META)?;
    if !companion_exists && !meta_exists {
        return Ok(None);
    }
    if companion_exists != meta_exists {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incomplete model identity pair (companion/meta mismatch)",
        ));
    }
    let companion_bytes = root.read_regular(MODEL_ROUTE_FILE)?;
    let meta_bytes = root.read_regular(MODEL_IDENTITY_META)?;
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
    // Digest the on-disk summary bytes (not a re-serialize).
    if root.exists_nofollow(SUMMARY_FILE)? {
        let summary_bytes = root.read_regular(SUMMARY_FILE)?;
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

/// Read `summary.json` via the multi-component dirfd walk (no path follow).
pub fn read_summary_contained(session_dir: &Path) -> io::Result<Summary> {
    let root = SessionRoot::open(session_dir)?;
    recover_identity_txn(&root)?;
    let bytes = root.read_regular(SUMMARY_FILE)?;
    if bytes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "summary.json is empty (0 bytes)",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Whether companion/meta pair files exist under the session dirfd.
pub fn identity_pair_present(session_dir: &Path) -> io::Result<bool> {
    let root = SessionRoot::open(session_dir)?;
    recover_identity_txn(&root)?;
    Ok(root.exists_nofollow(MODEL_ROUTE_FILE)? || root.exists_nofollow(MODEL_IDENTITY_META)?)
}

/// Commit summary + optional companion as one identity transaction.
/// Without a companion, the transaction commits summary only and removes both
/// identity artifacts so readers observe neither half of the pair.
/// `leave_on_mismatch` / Leave with mismatch fails closed.
///
/// Leave holds the identity lock across validate + digest rewrite so a
/// concurrent writer cannot observe a half-validated pair.
pub fn commit_summary_and_companion(
    session_dir: &Path,
    summary: &Summary,
    companion: Option<&ModelRouteProvenance>,
    leave_on_mismatch: bool,
) -> io::Result<()> {
    let root = SessionRoot::open(session_dir)?;
    let _lock = root.lock_exclusive()?;
    recover_identity_txn(&root)?;

    let previous_summary_bytes = if root.exists_nofollow(SUMMARY_FILE)? {
        Some(root.read_regular(SUMMARY_FILE)?)
    } else {
        None
    };

    if leave_on_mismatch && companion.is_none() {
        // Leave: validate existing pair against previous summary under the same
        // lock, then journal summary + meta digest update.
        let has_pair =
            root.exists_nofollow(MODEL_ROUTE_FILE)? || root.exists_nofollow(MODEL_IDENTITY_META)?;
        if has_pair {
            if let Some(prev) = &previous_summary_bytes {
                let prev_summary: Summary = serde_json::from_slice(prev).map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("summary: {e}"))
                })?;
                // Inline validate (same root/lock) — no nested re-open.
                validate_pair_against_summary(&root, &prev_summary)?;
            }
        }
        return commit_leave_digest_only_locked(&root, summary, previous_summary_bytes.as_deref());
    }

    commit_artifacts(&root, summary, companion, previous_summary_bytes.as_deref())
}

/// Validate companion/meta against `summary` using an already-open session root.
fn validate_pair_against_summary(root: &SessionRoot, summary: &Summary) -> io::Result<()> {
    let companion_exists = root.exists_nofollow(MODEL_ROUTE_FILE)?;
    let meta_exists = root.exists_nofollow(MODEL_IDENTITY_META)?;
    if !companion_exists && !meta_exists {
        return Ok(());
    }
    if companion_exists != meta_exists {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incomplete model identity pair (companion/meta mismatch)",
        ));
    }
    let companion_bytes = root.read_regular(MODEL_ROUTE_FILE)?;
    let meta_bytes = root.read_regular(MODEL_IDENTITY_META)?;
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
    if root.exists_nofollow(SUMMARY_FILE)? {
        let summary_bytes = root.read_regular(SUMMARY_FILE)?;
        if meta.summary_sha256 != sha256_hex(&summary_bytes) {
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
        if sha256_hex(&companion_bytes) != *expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "model_route companion digest mismatch",
            ));
        }
    }
    Ok(())
}

fn commit_leave_digest_only_locked(
    root: &SessionRoot,
    summary: &Summary,
    previous_summary_bytes: Option<&[u8]>,
) -> io::Result<()> {
    let summary_bytes = serde_json::to_vec_pretty(summary).map_err(io::Error::other)?;
    let summary_sha = sha256_hex(&summary_bytes);

    if !root.exists_nofollow(MODEL_IDENTITY_META)? {
        return stage_and_commit(
            root,
            Some((SUMMARY_TMP, SUMMARY_FILE, &summary_bytes, &summary_sha)),
            None,
            None,
            previous_summary_bytes.map(sha256_hex),
        );
    }

    let meta_bytes = root.read_regular(MODEL_IDENTITY_META)?;
    let mut meta: IdentityMeta = serde_json::from_slice(&meta_bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("meta: {e}")))?;
    meta.summary_sha256 = summary_sha.clone();
    let new_meta_bytes = serde_json::to_vec_pretty(&meta).map_err(io::Error::other)?;
    let meta_sha = sha256_hex(&new_meta_bytes);

    stage_and_commit(
        root,
        Some((SUMMARY_TMP, SUMMARY_FILE, &summary_bytes, &summary_sha)),
        None,
        Some((META_TMP, MODEL_IDENTITY_META, &new_meta_bytes, &meta_sha)),
        previous_summary_bytes.map(sha256_hex),
    )
}

fn commit_artifacts(
    root: &SessionRoot,
    summary: &Summary,
    companion: Option<&ModelRouteProvenance>,
    previous_summary_bytes: Option<&[u8]>,
) -> io::Result<()> {
    let summary_bytes = serde_json::to_vec_pretty(summary).map_err(io::Error::other)?;
    let summary_sha = sha256_hex(&summary_bytes);

    let Some(companion) = companion else {
        return commit_summary_without_provenance(
            root,
            &summary_bytes,
            &summary_sha,
            previous_summary_bytes,
        );
    };

    let pair_id = companion
        .pair_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut companion = companion.clone();
    if companion.pair_id.is_none() {
        companion = companion
            .with_pair_id(&pair_id)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
    }
    if companion.canonical_model.is_none() {
        companion.canonical_model = Some(summary.current_model_id.0.to_string());
    }
    let companion_bytes = serde_json::to_vec_pretty(&companion).map_err(io::Error::other)?;
    let companion_sha = sha256_hex(&companion_bytes);

    let meta = IdentityMeta {
        version: META_VERSION,
        pair_id: pair_id.clone(),
        canonical_model: summary.current_model_id.0.to_string(),
        summary_sha256: summary_sha.clone(),
        companion_sha256: Some(companion_sha.clone()),
    };
    let meta_bytes = serde_json::to_vec_pretty(&meta).map_err(io::Error::other)?;
    let meta_sha = sha256_hex(&meta_bytes);

    // Pre-clean staged names so exclusive create succeeds.
    for name in [SUMMARY_TMP, COMPANION_TMP, META_TMP] {
        let _ = root.unlink_nofollow(name);
    }

    root.write_staged(SUMMARY_TMP, &summary_bytes)?;
    root.write_staged(COMPANION_TMP, &companion_bytes)?;
    root.write_staged(META_TMP, &meta_bytes)?;
    root.revalidate()?;

    let marker = TxnMarker {
        version: TXN_VERSION,
        summary_tmp: Some(SUMMARY_TMP.into()),
        companion_tmp: Some(COMPANION_TMP.into()),
        meta_tmp: Some(META_TMP.into()),
        new_summary_sha: Some(summary_sha),
        new_companion_sha: Some(companion_sha),
        new_meta_sha: Some(meta_sha),
        previous_summary_sha: previous_summary_bytes.map(sha256_hex),
        ready_to_commit: true,
    };
    write_txn_marker(root, &marker)?;

    // Commit order: summary → companion → meta, then drop journal.
    root.rename_nofollow(SUMMARY_TMP, SUMMARY_FILE)?;
    root.rename_nofollow(COMPANION_TMP, MODEL_ROUTE_FILE)?;
    root.rename_nofollow(META_TMP, MODEL_IDENTITY_META)?;
    clear_txn_marker(root)?;
    let _ = root.fsync_dir();
    Ok(())
}

fn commit_summary_without_provenance(
    root: &SessionRoot,
    summary_bytes: &[u8],
    summary_sha: &str,
    previous_summary_bytes: Option<&[u8]>,
) -> io::Result<()> {
    for name in [SUMMARY_TMP, COMPANION_TMP, META_TMP] {
        root.unlink_nofollow(name)?;
    }
    root.write_staged(SUMMARY_TMP, summary_bytes)?;
    root.revalidate()?;

    let marker = TxnMarker {
        version: TXN_VERSION,
        summary_tmp: Some(SUMMARY_TMP.into()),
        companion_tmp: None,
        meta_tmp: None,
        new_summary_sha: Some(summary_sha.to_owned()),
        new_companion_sha: None,
        new_meta_sha: None,
        previous_summary_sha: previous_summary_bytes.map(sha256_hex),
        ready_to_commit: true,
    };
    write_txn_marker(root, &marker)?;

    root.rename_nofollow(SUMMARY_TMP, SUMMARY_FILE)?;
    root.fsync_dir()?;
    root.unlink_nofollow(MODEL_ROUTE_FILE)?;
    root.fsync_dir()?;
    root.unlink_nofollow(MODEL_IDENTITY_META)?;
    root.fsync_dir()?;
    clear_txn_marker(root)?;
    root.fsync_dir()?;
    Ok(())
}

fn stage_and_commit(
    root: &SessionRoot,
    summary: Option<(&str, &str, &[u8], &str)>,
    companion: Option<(&str, &str, &[u8], &str)>,
    meta: Option<(&str, &str, &[u8], &str)>,
    previous_summary_sha: Option<String>,
) -> io::Result<()> {
    for name in [SUMMARY_TMP, COMPANION_TMP, META_TMP] {
        let _ = root.unlink_nofollow(name);
    }
    if let Some((tmp, _, bytes, _)) = summary {
        root.write_staged(tmp, bytes)?;
    }
    if let Some((tmp, _, bytes, _)) = companion {
        root.write_staged(tmp, bytes)?;
    }
    if let Some((tmp, _, bytes, _)) = meta {
        root.write_staged(tmp, bytes)?;
    }
    root.revalidate()?;

    let marker = TxnMarker {
        version: TXN_VERSION,
        summary_tmp: summary.map(|(t, _, _, _)| t.to_owned()),
        companion_tmp: companion.map(|(t, _, _, _)| t.to_owned()),
        meta_tmp: meta.map(|(t, _, _, _)| t.to_owned()),
        new_summary_sha: summary.map(|(_, _, _, s)| s.to_owned()),
        new_companion_sha: companion.map(|(_, _, _, s)| s.to_owned()),
        new_meta_sha: meta.map(|(_, _, _, s)| s.to_owned()),
        previous_summary_sha,
        ready_to_commit: true,
    };
    write_txn_marker(root, &marker)?;

    if let Some((tmp, final_name, _, _)) = summary {
        root.rename_nofollow(tmp, final_name)?;
    }
    if let Some((tmp, final_name, _, _)) = companion {
        root.rename_nofollow(tmp, final_name)?;
    }
    if let Some((tmp, final_name, _, _)) = meta {
        root.rename_nofollow(tmp, final_name)?;
    }
    clear_txn_marker(root)?;
    let _ = root.fsync_dir();
    Ok(())
}

/// Clear companion when model changes without new provenance.
pub fn clear_route_companion(session_dir: &Path) -> io::Result<()> {
    let root = SessionRoot::open(session_dir)?;
    let _lock = root.lock_exclusive()?;
    recover_identity_txn(&root)?;
    for name in [MODEL_ROUTE_FILE, MODEL_IDENTITY_META, MODEL_IDENTITY_TXN] {
        root.unlink_nofollow(name)?;
    }
    let _ = root.fsync_dir();
    Ok(())
}

/// Copy companion+meta from source session dir into target, rebinding digests
/// to the already-written target summary. Used by fork/copy. No rewrite of
/// the target summary. Fails closed on source pair mismatch.
pub fn copy_route_companion_for_fork(
    source_session_dir: &Path,
    target_session_dir: &Path,
    target_summary: &Summary,
) -> io::Result<()> {
    // Load against source summary on disk so digests match source pair.
    let source_root = SessionRoot::open(source_session_dir)?;
    recover_identity_txn(&source_root)?;
    if !source_root.exists_nofollow(MODEL_ROUTE_FILE)?
        && !source_root.exists_nofollow(MODEL_IDENTITY_META)?
    {
        return Ok(());
    }
    if source_root.exists_nofollow(SUMMARY_FILE)? {
        let source_summary_bytes = source_root.read_regular(SUMMARY_FILE)?;
        let source_summary: Summary = serde_json::from_slice(&source_summary_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let companion = load_route_companion(source_session_dir, &source_summary)?;
        if let Some(mut prov) = companion {
            // Fresh pair id for the fork so source/target cannot share identity.
            let new_pair = uuid::Uuid::new_v4().to_string();
            prov = prov
                .with_pair_id(&new_pair)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
            // Rebind canonical to the target summary's current model when present.
            if let Ok(canon) =
                xai_grok_models::CanonicalModelId::new(target_summary.current_model_id.0.as_ref())
            {
                prov = prov.with_canonical_model(&canon);
            }
            commit_summary_and_companion(target_session_dir, target_summary, Some(&prov), false)?;
        }
    }
    Ok(())
}

/// Build secret-free provenance from a live production route context.
pub fn provenance_from_route_context(
    route: &xai_grok_inference::ProviderRouteContext,
    canonical_model: &str,
    upstream_model: &str,
) -> Option<ModelRouteProvenance> {
    let upstream = xai_grok_models::UpstreamModelId::new(upstream_model).ok()?;
    let mut prov = ModelRouteProvenance::new(
        route.instance_id(),
        route.incarnation(),
        Some(route.provider_kind().as_str()),
        Some(route.api_surface().as_str()),
        &upstream,
        // Exact routes require nonzero generation; legacy (no incarnation) allows 0.
        if route.incarnation().is_some() {
            route.registry_generation().max(1)
        } else {
            route.registry_generation()
        },
    )
    .ok()?;
    if let Ok(canon) = xai_grok_models::CanonicalModelId::new(canonical_model) {
        prov = prov.with_canonical_model(&canon);
    }
    Some(prov)
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

    #[test]
    fn no_provenance_commit_writes_summary_without_identity_pair() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("sess");
        fs::create_dir_all(&session).unwrap();
        let summary = sample_summary("grok-4.5");

        commit_summary_and_companion(&session, &summary, None, false).unwrap();

        assert_eq!(
            read_summary_contained(&session).unwrap().current_model_id,
            summary.current_model_id
        );
        assert!(!session.join(MODEL_ROUTE_FILE).exists());
        assert!(!session.join(MODEL_IDENTITY_META).exists());
        assert!(load_route_companion(&session, &summary).unwrap().is_none());
    }

    #[test]
    fn no_provenance_recovery_removes_both_identity_artifacts() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("sess");
        fs::create_dir_all(&session).unwrap();
        let summary = sample_summary("openai-gpt-4o");
        commit_summary_and_companion(
            &session,
            &summary,
            Some(&provenance("openai-gpt-4o")),
            false,
        )
        .unwrap();

        let mut next = summary.clone();
        next.current_model_id = acp::ModelId::new("grok-4.5");
        let summary_bytes = serde_json::to_vec_pretty(&next).unwrap();
        let summary_sha = sha256_hex(&summary_bytes);
        fs::write(session.join(SUMMARY_TMP), &summary_bytes).unwrap();
        let marker = TxnMarker {
            version: TXN_VERSION,
            summary_tmp: Some(SUMMARY_TMP.into()),
            companion_tmp: None,
            meta_tmp: None,
            new_summary_sha: Some(summary_sha),
            new_companion_sha: None,
            new_meta_sha: None,
            previous_summary_sha: Some(sha256_hex(&fs::read(session.join(SUMMARY_FILE)).unwrap())),
            ready_to_commit: true,
        };
        fs::write(
            session.join(MODEL_IDENTITY_TXN),
            serde_json::to_vec_pretty(&marker).unwrap(),
        )
        .unwrap();

        assert_eq!(
            read_summary_contained(&session).unwrap().current_model_id,
            next.current_model_id
        );
        assert!(!session.join(MODEL_ROUTE_FILE).exists());
        assert!(!session.join(MODEL_IDENTITY_META).exists());
        assert!(load_route_companion(&session, &next).unwrap().is_none());
    }

    #[test]
    fn dest_symlink_refused_on_read() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("sess");
        fs::create_dir_all(&session).unwrap();
        let summary = sample_summary("openai-gpt-4o");
        let outside = dir.path().join("outside.json");
        fs::write(&outside, b"{\"evil\":true}").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, session.join(MODEL_ROUTE_FILE)).unwrap();
            std::os::unix::fs::symlink(&outside, session.join(MODEL_IDENTITY_META)).unwrap();
            let err = load_route_companion(&session, &summary).unwrap_err();
            // O_NOFOLLOW → ELOOP / InvalidData
            assert!(
                err.kind() == io::ErrorKind::InvalidData
                    || err.raw_os_error() == Some(libc::ELOOP)
                    || err.kind() == io::ErrorKind::Other,
                "unexpected err: {err:?}"
            );
        }
    }

    #[test]
    fn ancestor_symlink_session_root_refused() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("real_sess");
        fs::create_dir_all(&real).unwrap();
        let link = dir.path().join("link_sess");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&real, &link).unwrap();
            // Final component is a symlink → multi-component walk fails.
            match SessionRoot::open(&link) {
                Ok(_) => panic!("symlink session root must be refused"),
                Err(err) => assert!(
                    err.raw_os_error() == Some(libc::ELOOP)
                        || err.raw_os_error() == Some(libc::ENOTDIR)
                        || err.kind() == io::ErrorKind::NotADirectory
                        || err.kind() == io::ErrorKind::InvalidData
                        || err.kind() == io::ErrorKind::PermissionDenied
                        || err.kind() == io::ErrorKind::Other,
                    "unexpected err: {err:?}"
                ),
            }
        }
    }

    /// Intermediate `sessions` component is a symlink to an attacker dir —
    /// multi-component walk must fail closed (Gate A Issue 1).
    #[cfg(unix)]
    #[test]
    fn intermediate_sessions_symlink_refused() {
        let dir = tempdir().unwrap();
        let evil = dir.path().join("evil").join("sessions").join("sess");
        fs::create_dir_all(&evil).unwrap();
        let sessions_link = dir.path().join("sessions");
        std::os::unix::fs::symlink(dir.path().join("evil").join("sessions"), &sessions_link)
            .unwrap();
        // Real layout under the link target: sessions/sess, but `sessions` is a symlink.
        let attacked = dir.path().join("sessions").join("sess");
        match SessionRoot::open(&attacked) {
            Ok(_) => panic!("intermediate sessions symlink must be refused"),
            Err(err) => assert!(
                err.raw_os_error() == Some(libc::ELOOP)
                    || err.raw_os_error() == Some(libc::ENOTDIR)
                    || err.kind() == io::ErrorKind::NotADirectory
                    || err.kind() == io::ErrorKind::InvalidData
                    || err.kind() == io::ErrorKind::PermissionDenied
                    || err.kind() == io::ErrorKind::Other,
                "unexpected err: {err:?}"
            ),
        }
    }

    /// Contained summary read must not path-follow a `sessions` symlink to
    /// adopt attacker content (final Gate A residual finding).
    #[cfg(unix)]
    #[test]
    fn intermediate_sessions_symlink_read_summary_fails_closed() {
        let dir = tempdir().unwrap();
        let evil_sess = dir.path().join("evil").join("sessions").join("sess");
        fs::create_dir_all(&evil_sess).unwrap();
        let attacker_bytes =
            br#"{"current_model_id":"attacker-slug","info":{"id":"sess","cwd":"/tmp"}}"#;
        fs::write(evil_sess.join(SUMMARY_FILE), attacker_bytes).unwrap();
        std::os::unix::fs::symlink(
            dir.path().join("evil").join("sessions"),
            dir.path().join("sessions"),
        )
        .unwrap();
        let attacked = dir.path().join("sessions").join("sess");
        // Path follow would see the attacker file; contained walk must refuse.
        let err = read_summary_contained(&attacked).unwrap_err();
        assert!(
            err.raw_os_error() == Some(libc::ELOOP)
                || err.raw_os_error() == Some(libc::ENOTDIR)
                || err.kind() == io::ErrorKind::NotADirectory
                || err.kind() == io::ErrorKind::InvalidData
                || err.kind() == io::ErrorKind::PermissionDenied
                || err.kind() == io::ErrorKind::Other
                || err.kind() == io::ErrorKind::NotFound,
            "unexpected err: {err:?}"
        );
        // Attacker content still only on the evil side — not adopted as Ok(Summary).
        assert_eq!(
            fs::read(evil_sess.join(SUMMARY_FILE)).unwrap(),
            attacker_bytes
        );
    }

    /// Model switch installs a pair; ordinary Leave rewrite (chat-style)
    /// must keep the companion loadable (Gate A Issue 2).
    #[test]
    fn ordinary_summary_leave_preserves_companion_digest() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("sess");
        fs::create_dir_all(&session).unwrap();
        let summary = sample_summary("openai-gpt-4o");
        let prov = provenance("openai-gpt-4o");
        commit_summary_and_companion(&session, &summary, Some(&prov), false).unwrap();
        // Simulate chat append: bump counters via Leave.
        let mut next = summary.clone();
        next.num_messages = 1;
        next.num_chat_messages = 1;
        commit_summary_and_companion(&session, &next, None, true).unwrap();
        let loaded = load_route_companion(&session, &next).unwrap().unwrap();
        assert_eq!(
            loaded.pair_id.as_deref(),
            Some("pair-token-aaaaaaaaaaaaaaaa")
        );
        assert_eq!(loaded.canonical_model.as_deref(), Some("openai-gpt-4o"));
    }

    #[test]
    fn crash_after_summary_rename_before_companion_rolls_forward_or_back() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("sess");
        fs::create_dir_all(&session).unwrap();
        let summary = sample_summary("openai-gpt-4o");
        let prov = provenance("openai-gpt-4o");
        // Establish a previous pair.
        commit_summary_and_companion(&session, &summary, Some(&prov), false).unwrap();
        let prev_companion = fs::read(session.join(MODEL_ROUTE_FILE)).unwrap();

        // Simulate a crash mid-commit: journal ready + staged temps present,
        // finals still old. Recovery must either roll forward fully or roll back.
        let mut next = summary.clone();
        next.num_messages = 9;
        let summary_bytes = serde_json::to_vec_pretty(&next).unwrap();
        let summary_sha = sha256_hex(&summary_bytes);
        let mut new_prov = provenance("openai-gpt-4o");
        new_prov = new_prov
            .with_pair_id("pair-token-cccccccccccccccc")
            .unwrap();
        let companion_bytes = serde_json::to_vec_pretty(&new_prov).unwrap();
        let companion_sha = sha256_hex(&companion_bytes);
        let meta = IdentityMeta {
            version: META_VERSION,
            pair_id: "pair-token-cccccccccccccccc".into(),
            canonical_model: "openai-gpt-4o".into(),
            summary_sha256: summary_sha.clone(),
            companion_sha256: Some(companion_sha.clone()),
        };
        let meta_bytes = serde_json::to_vec_pretty(&meta).unwrap();
        let meta_sha = sha256_hex(&meta_bytes);

        fs::write(session.join(SUMMARY_TMP), &summary_bytes).unwrap();
        fs::write(session.join(COMPANION_TMP), &companion_bytes).unwrap();
        fs::write(session.join(META_TMP), &meta_bytes).unwrap();
        let marker = TxnMarker {
            version: TXN_VERSION,
            summary_tmp: Some(SUMMARY_TMP.into()),
            companion_tmp: Some(COMPANION_TMP.into()),
            meta_tmp: Some(META_TMP.into()),
            new_summary_sha: Some(summary_sha),
            new_companion_sha: Some(companion_sha),
            new_meta_sha: Some(meta_sha),
            previous_summary_sha: Some(sha256_hex(&fs::read(session.join(SUMMARY_FILE)).unwrap())),
            ready_to_commit: true,
        };
        fs::write(
            session.join(MODEL_IDENTITY_TXN),
            serde_json::to_vec_pretty(&marker).unwrap(),
        )
        .unwrap();

        // Next open recovers.
        let loaded = load_route_companion(&session, &next).unwrap().unwrap();
        assert_eq!(
            loaded.pair_id.as_deref(),
            Some("pair-token-cccccccccccccccc")
        );
        // Incomplete crash (only summary tmp, ready=false) must not mix.
        fs::write(session.join(MODEL_ROUTE_FILE), &prev_companion).unwrap();
    }

    #[test]
    fn incomplete_txn_without_ready_rolls_back_keeps_previous() {
        let dir = tempdir().unwrap();
        let session = dir.path().join("sess");
        fs::create_dir_all(&session).unwrap();
        let summary = sample_summary("openai-gpt-4o");
        let prov = provenance("openai-gpt-4o");
        commit_summary_and_companion(&session, &summary, Some(&prov), false).unwrap();
        let prev_summary = fs::read(session.join(SUMMARY_FILE)).unwrap();
        let prev_companion = fs::read(session.join(MODEL_ROUTE_FILE)).unwrap();

        // Crash after writing only a new summary temp; not ready_to_commit.
        fs::write(session.join(SUMMARY_TMP), b"{\"partial\":true}").unwrap();
        let marker = TxnMarker {
            version: TXN_VERSION,
            summary_tmp: Some(SUMMARY_TMP.into()),
            companion_tmp: None,
            meta_tmp: None,
            new_summary_sha: Some(sha256_hex(b"{\"partial\":true}")),
            new_companion_sha: None,
            new_meta_sha: None,
            previous_summary_sha: Some(sha256_hex(&prev_summary)),
            ready_to_commit: false,
        };
        fs::write(
            session.join(MODEL_IDENTITY_TXN),
            serde_json::to_vec_pretty(&marker).unwrap(),
        )
        .unwrap();

        let loaded = load_route_companion(&session, &summary).unwrap().unwrap();
        assert_eq!(
            loaded.pair_id.as_deref(),
            Some("pair-token-aaaaaaaaaaaaaaaa")
        );
        assert_eq!(fs::read(session.join(SUMMARY_FILE)).unwrap(), prev_summary);
        assert_eq!(
            fs::read(session.join(MODEL_ROUTE_FILE)).unwrap(),
            prev_companion
        );
        assert!(!session.join(SUMMARY_TMP).exists());
        assert!(!session.join(MODEL_IDENTITY_TXN).exists());
    }

    #[cfg(unix)]
    #[test]
    fn toctou_parent_swap_detected_on_revalidate() {
        // Build a real root, open it, then replace the directory path with a
        // different directory (simulate rename-swap of the session dir).
        let dir = tempdir().unwrap();
        let session = dir.path().join("sess");
        fs::create_dir_all(&session).unwrap();
        let root = SessionRoot::open(&session).unwrap();
        // Replace session with a new directory at the same path.
        fs::remove_dir_all(&session).unwrap();
        fs::create_dir_all(&session).unwrap();
        let err = root.revalidate().unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
