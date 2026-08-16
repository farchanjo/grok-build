//! Owner-only durable binary writes for platform CLI sinks.
//!
//! Shared by transport `execute_binary` sinks and the shell CLI `write_binary`
//! path so every durable binary destination uses one hardened primitive:
//! unique temp, mode 0600, flush/sync, atomic rename, parent sync where
//! supported, symlink refusal, and Drop cleanup of incomplete temps.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static WRITE_NONCE: AtomicU64 = AtomicU64::new(0);

/// Remove a temp path on drop unless disarmed after a successful rename.
struct TempCleanup(Option<PathBuf>);

impl TempCleanup {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for TempCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_file(path);
        }
    }
}

/// Write `bytes` to `final_path` with owner-only durable semantics.
///
/// - Refuses when `final_path` is an existing symlink (target-safety).
/// - Creates a unique sibling temp with `create_new` and mode `0600`.
/// - Flushes and `sync_all`s the temp before rename.
/// - Atomically renames temp → final.
/// - Best-effort parent directory sync after rename.
/// - Drops (unlinks) the temp if any step fails or the process panics mid-write.
pub fn write_owner_only_atomic(final_path: &Path, bytes: &[u8]) -> io::Result<()> {
    if final_path.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "binary output path is empty",
        ));
    }

    // Symlink/target safety: never clobber or follow a final path that is a symlink.
    match fs::symlink_metadata(final_path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refusing to write binary output through a symlink",
            ));
        }
        Ok(_) | Err(_) => {}
    }

    let parent = final_path.parent().filter(|p| !p.as_os_str().is_empty());
    let parent = parent.unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let file_name = final_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "binary.bin".to_owned());
    let nonce = WRITE_NONCE.fetch_add(1, Ordering::Relaxed);
    let tmp = parent.join(format!(".{file_name}.{}.{nonce}.tmp", std::process::id()));

    let mut cleanup = TempCleanup(Some(tmp.clone()));

    {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(&tmp)?;
        let mut perms = meta.permissions();
        if perms.mode() & 0o777 != 0o600 {
            perms.set_mode(0o600);
            fs::set_permissions(&tmp, perms)?;
        }
    }

    fs::rename(&tmp, final_path)?;
    cleanup.disarm();

    // Parent sync keeps directory entries durable on crash-prone filesystems.
    #[cfg(unix)]
    {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
        // Re-assert owner-only on the final path after rename.
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(final_path) {
            let mut perms = meta.permissions();
            if perms.mode() & 0o777 != 0o600 {
                perms.set_mode(0o600);
                let _ = fs::set_permissions(final_path, perms);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "grok-durable-write-{}-{}-{}",
            label,
            std::process::id(),
            nanos
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_owner_only_contents_atomically() {
        let dir = test_dir("ok");
        let path = dir.join("out.bin");
        write_owner_only_atomic(&path, b"hello-binary").unwrap();
        let mut got = Vec::new();
        fs::File::open(&path)
            .unwrap()
            .read_to_end(&mut got)
            .unwrap();
        assert_eq!(got, b"hello-binary");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "final file must be owner-only");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn refuses_symlink_final_path() {
        let dir = test_dir("symlink");
        let target = dir.join("real.bin");
        fs::write(&target, b"x").unwrap();
        let link = dir.join("link.bin");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target, &link).unwrap();
            let err = write_owner_only_atomic(&link, b"nope").unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
            assert_eq!(fs::read(&target).unwrap(), b"x");
        }
        #[cfg(not(unix))]
        {
            let _ = (target, link);
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn failure_cleans_temp_and_leaves_no_partial_final() {
        let dir = test_dir("fail");
        // Point final path at a nested name under a file (not a directory).
        let blocker = dir.join("not-a-dir");
        fs::write(&blocker, b"file").unwrap();
        let path = blocker.join("out.bin");
        let err = write_owner_only_atomic(&path, b"data");
        assert!(err.is_err());
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files must be cleaned on failure: {leftovers:?}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn overwrites_existing_regular_file() {
        let dir = test_dir("overwrite");
        let path = dir.join("out.bin");
        fs::write(&path, b"old").unwrap();
        write_owner_only_atomic(&path, b"new-content").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new-content");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let _ = fs::remove_dir_all(dir);
    }
}
