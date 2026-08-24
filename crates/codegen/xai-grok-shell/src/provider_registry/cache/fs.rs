//! Owner-only, no-follow filesystem primitives for provider cache storage.
//!
//! Paths are opened relative to a trusted root with single-component names.
//! Sensitive mutations never rely on process-global path joins alone.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use fs2::FileExt;

use crate::session::storage::sync_file_durable;

pub(super) const PROVIDER_CACHES_DIR: &str = "provider_caches";
pub(super) const CATALOG_FILE: &str = "catalog.json";
pub(super) const CAPABILITIES_FILE: &str = "capabilities.json";
pub(super) const STATE_FILE: &str = "state.json";
pub(super) const LOCK_FILE: &str = "provider_cache.lock";
pub(super) const TXN_FILE: &str = "provider_cache.txn";

pub(super) const MAX_CATALOG_BYTES: u64 = 8 * 1024 * 1024;
pub(super) const MAX_CAPABILITIES_BYTES: u64 = 1 * 1024 * 1024;
pub(super) const MAX_STATE_BYTES: u64 = 64 * 1024;
pub(super) const MAX_TXN_BYTES: u64 = 16 * 1024;
pub(super) const MAX_TEMP_NAME_BYTES: usize = 96;

pub(super) fn invalid_input(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

pub(super) fn invalid_data(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// Trusted open of `$GROK_HOME/provider_caches/<instance-id>/`.
pub(super) struct TrustedInstanceDir {
    pub(super) path: PathBuf,
    #[cfg(unix)]
    pub(super) dir: File,
    #[cfg(unix)]
    pub(super) root: File,
}

impl TrustedInstanceDir {
    pub(super) fn open(
        grok_home: &Path,
        instance_component: &str,
        create: bool,
    ) -> io::Result<Self> {
        validate_single_component_name(instance_component)?;
        validate_instance_id_component(instance_component)?;

        #[cfg(unix)]
        {
            let home_dir = open_directory_nofollow(grok_home)?;
            validate_dir_fd(&home_dir, false)?;
            let root = openat_or_mkdir_directory(&home_dir, PROVIDER_CACHES_DIR)?;
            validate_dir_fd(&root, true)?;
            let dir = if create {
                openat_or_mkdir_directory(&root, instance_component)?
            } else {
                openat_directory(&root, instance_component)?
            };
            validate_dir_fd(&dir, true)?;
            let path = grok_home.join(PROVIDER_CACHES_DIR).join(instance_component);
            Ok(Self { path, dir, root })
        }
        #[cfg(not(unix))]
        {
            let root = grok_home.join(PROVIDER_CACHES_DIR);
            let path = root.join(instance_component);
            if create {
                std::fs::create_dir_all(&path)?;
            } else if !path.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "provider cache instance directory missing",
                ));
            }
            validate_real_directory(&root)?;
            validate_real_directory(&path)?;
            Ok(Self { path })
        }
    }
}

pub(super) fn validate_single_component_name(name: &str) -> io::Result<()> {
    if name.is_empty() {
        return Err(invalid_input("empty path component"));
    }
    if name.len() > 255 {
        return Err(invalid_input("path component too long"));
    }
    if name.contains('\0') {
        return Err(invalid_input("path component contains NUL"));
    }
    if name == "." || name == ".." {
        return Err(invalid_input("path component must not be . or .."));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(invalid_input("path component must be a single segment"));
    }
    if name.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err(invalid_input("path component contains control characters"));
    }
    Ok(())
}

fn validate_instance_id_component(name: &str) -> io::Result<()> {
    if Path::new(name).components().count() != 1 {
        return Err(invalid_input("instance id is not a single path component"));
    }
    if let Some(Component::Normal(_)) = Path::new(name).components().next() {
        Ok(())
    } else {
        Err(invalid_input("instance id is not a normal path component"))
    }
}

#[cfg(unix)]
fn open_directory_nofollow(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_RDONLY);
    options.open(path)
}

#[cfg(unix)]
fn openat_directory(parent: &File, name: &str) -> io::Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    validate_single_component_name(name)?;
    let cname = std::ffi::CString::new(name.as_bytes())
        .map_err(|_| invalid_input("path component contains NUL"))?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            cname.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(unix)]
fn openat_or_mkdir_directory(parent: &File, name: &str) -> io::Result<File> {
    for attempt in 0..4 {
        match openat_directory(parent, name) {
            Ok(dir) => return Ok(dir),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                match mkdirat_0700(parent, name) {
                    Ok(()) => {}
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e),
                }
                match openat_directory(parent, name) {
                    Ok(dir) => return Ok(dir),
                    Err(e) if e.kind() == io::ErrorKind::NotFound && attempt + 1 < 4 => {
                        std::thread::yield_now();
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "provider cache directory disappeared during create",
    ))
}

#[cfg(unix)]
fn mkdirat_0700(parent: &File, name: &str) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;
    validate_single_component_name(name)?;
    let cname = std::ffi::CString::new(name.as_bytes())
        .map_err(|_| invalid_input("path component contains NUL"))?;
    let rc = unsafe { libc::mkdirat(parent.as_raw_fd(), cname.as_ptr(), 0o700) };
    if rc != 0 {
        let err = io::Error::last_os_error();
        if err.kind() == io::ErrorKind::AlreadyExists {
            return Ok(());
        }
        return Err(err);
    }
    Ok(())
}

#[cfg(unix)]
fn validate_dir_fd(dir: &File, require_owner_only: bool) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let meta = dir.metadata()?;
    if !meta.is_dir() {
        return Err(invalid_input("provider cache path is not a directory"));
    }
    let uid = unsafe { libc::geteuid() };
    if meta.uid() != uid {
        return Err(invalid_input(
            "provider cache directory has unexpected owner",
        ));
    }
    let mode = meta.mode() & 0o777;
    if mode & 0o022 != 0 {
        return Err(invalid_input(
            "provider cache directory must not be group/other-writable",
        ));
    }
    if require_owner_only && mode & 0o077 != 0 {
        use std::os::fd::AsRawFd;
        if unsafe { libc::fchmod(dir.as_raw_fd(), 0o700) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let meta = dir.metadata()?;
        let mode = meta.mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(invalid_input(
                "provider cache directory must be owner-only (0700)",
            ));
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_real_directory(path: &Path) -> io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(invalid_input("provider cache path is not a directory"));
    }
    Ok(())
}

pub(super) struct InstanceLock {
    file: File,
    #[cfg(unix)]
    ino: u64,
    #[cfg(unix)]
    dev: u64,
}

impl InstanceLock {
    pub(super) fn acquire(instance: &TrustedInstanceDir) -> io::Result<Self> {
        validate_single_component_name(LOCK_FILE)?;
        let file = open_lock_relative(instance, LOCK_FILE)?;
        file.lock_exclusive()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = file.metadata()?;
            let lock = Self {
                file,
                ino: meta.ino(),
                dev: meta.dev(),
            };
            if !lock.still_live(instance) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "provider cache lock is no longer live",
                ));
            }
            Ok(lock)
        }
        #[cfg(not(unix))]
        {
            Ok(Self { file })
        }
    }

    #[cfg(unix)]
    pub(super) fn still_live(&self, instance: &TrustedInstanceDir) -> bool {
        use std::os::unix::fs::MetadataExt;
        let Ok(path_file) = open_existing_regular_relative(instance, LOCK_FILE) else {
            return false;
        };
        let Ok(path_meta) = path_file.metadata() else {
            return false;
        };
        path_meta.ino() == self.ino && path_meta.dev() == self.dev
    }

    #[cfg(not(unix))]
    pub(super) fn still_live(&self, _instance: &TrustedInstanceDir) -> bool {
        true
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[cfg(unix)]
fn open_lock_relative(instance: &TrustedInstanceDir, name: &str) -> io::Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    match fstatat_relative(instance, name) {
        Ok(meta) => {
            if meta.is_symlink {
                return Err(invalid_input("provider cache lock must not be a symlink"));
            }
            if !meta.is_file {
                return Err(invalid_input("provider cache lock is not a regular file"));
            }
            let uid = unsafe { libc::geteuid() };
            if meta.uid != uid {
                return Err(invalid_input("provider cache lock has unexpected owner"));
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let cname = std::ffi::CString::new(name.as_bytes())
        .map_err(|_| invalid_input("lock name contains NUL"))?;
    let fd = unsafe {
        libc::openat(
            instance.dir.as_raw_fd(),
            cname.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let opened = file.metadata()?;
    if !opened.is_file() {
        return Err(invalid_input("provider cache lock is not a regular file"));
    }
    if unsafe { libc::fchmod(file.as_raw_fd(), 0o600) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(file)
}

#[cfg(not(unix))]
fn open_lock_relative(instance: &TrustedInstanceDir, name: &str) -> io::Result<File> {
    let path = instance.path.join(name);
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

#[cfg(unix)]
struct StatMeta {
    uid: u32,
    is_file: bool,
    is_symlink: bool,
    is_dir: bool,
}

#[cfg(unix)]
fn fstatat_relative(instance: &TrustedInstanceDir, name: &str) -> io::Result<StatMeta> {
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;
    validate_single_component_name(name)?;
    let cname =
        std::ffi::CString::new(name.as_bytes()).map_err(|_| invalid_input("name contains NUL"))?;
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let rc = unsafe {
        libc::fstatat(
            instance.dir.as_raw_fd(),
            cname.as_ptr(),
            &mut st,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    let ft = st.st_mode & libc::S_IFMT;
    Ok(StatMeta {
        uid: st.st_uid as u32,
        is_file: ft == libc::S_IFREG,
        is_symlink: ft == libc::S_IFLNK,
        is_dir: ft == libc::S_IFDIR,
    })
}

#[cfg(unix)]
fn open_existing_regular_relative(instance: &TrustedInstanceDir, name: &str) -> io::Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let meta = fstatat_relative(instance, name)?;
    if meta.is_symlink {
        return Err(invalid_input("refusing symlink"));
    }
    if !meta.is_file {
        return Err(invalid_input("expected regular file"));
    }
    let uid = unsafe { libc::geteuid() };
    if meta.uid != uid {
        return Err(invalid_input("cache file has unexpected owner"));
    }
    let cname =
        std::ffi::CString::new(name.as_bytes()).map_err(|_| invalid_input("name contains NUL"))?;
    let fd = unsafe {
        libc::openat(
            instance.dir.as_raw_fd(),
            cname.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

pub(super) fn read_optional_regular_relative(
    instance: &TrustedInstanceDir,
    name: &str,
    max_bytes: u64,
) -> io::Result<Option<Vec<u8>>> {
    validate_single_component_name(name)?;
    #[cfg(unix)]
    {
        match fstatat_relative(instance, name) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
            Ok(meta) if meta.is_symlink => {
                return Err(invalid_input("refusing symlink"));
            }
            Ok(meta) if !meta.is_file => return Err(invalid_input("expected regular file")),
            Ok(meta) => {
                let uid = unsafe { libc::geteuid() };
                if meta.uid != uid {
                    return Err(invalid_input("cache file has unexpected owner"));
                }
            }
        }
        let mut file = open_existing_regular_relative(instance, name)?;
        let opened = file.metadata()?;
        if !opened.is_file() || opened.len() > max_bytes {
            return Err(invalid_data("cache file size invalid or exceeds bound"));
        }
        let mut bytes = Vec::with_capacity(opened.len() as usize);
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > max_bytes {
            return Err(invalid_data("cache file exceeds bound"));
        }
        Ok(Some(bytes))
    }
    #[cfg(not(unix))]
    {
        let path = instance.path.join(name);
        let meta = match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
            Ok(m) => m,
        };
        if meta.file_type().is_symlink() || !meta.is_file() {
            return Err(invalid_input("expected regular file"));
        }
        if meta.len() > max_bytes {
            return Err(invalid_data("cache file exceeds bound"));
        }
        let bytes = std::fs::read(&path)?;
        if bytes.len() as u64 > max_bytes {
            return Err(invalid_data("cache file exceeds bound"));
        }
        Ok(Some(bytes))
    }
}

pub(super) fn stage_bytes_relative(
    instance: &TrustedInstanceDir,
    final_name: &str,
    bytes: &[u8],
) -> io::Result<String> {
    validate_single_component_name(final_name)?;
    let tmp_name = format!(
        ".{final_name}.{}.{}.tmp",
        std::process::id(),
        uuid::Uuid::now_v7().simple()
    );
    if tmp_name.len() > MAX_TEMP_NAME_BYTES {
        return Err(invalid_input("temporary name too long"));
    }
    validate_single_component_name(&tmp_name)?;

    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::ffi::OsStrExt;
        let cname = std::ffi::CString::new(tmp_name.as_bytes())
            .map_err(|_| invalid_input("tmp name contains NUL"))?;
        let fd = unsafe {
            libc::openat(
                instance.dir.as_raw_fd(),
                cname.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        let result = (|| {
            file.write_all(bytes)?;
            sync_file_durable(&file)?;
            Ok::<(), io::Error>(())
        })();
        if result.is_err() {
            let _ = unlink_relative(instance, &tmp_name);
            result?;
        }
        Ok(tmp_name)
    }
    #[cfg(not(unix))]
    {
        let tmp_path = instance.path.join(&tmp_name);
        let result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            let mut file = options.open(&tmp_path)?;
            file.write_all(bytes)?;
            sync_file_durable(&file)?;
            Ok::<(), io::Error>(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp_path);
            result?;
        }
        Ok(tmp_name)
    }
}

pub(super) fn rename_relative(
    instance: &TrustedInstanceDir,
    from: &str,
    to: &str,
) -> io::Result<()> {
    validate_single_component_name(from)?;
    validate_single_component_name(to)?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStrExt;
        match fstatat_relative(instance, to) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
            Ok(meta) if meta.is_symlink => {
                return Err(invalid_input("refusing to replace a symlink destination"));
            }
            Ok(meta) if meta.is_dir => {
                return Err(invalid_input("refusing to replace a directory destination"));
            }
            Ok(_) => {}
        }
        let cfrom = std::ffi::CString::new(from.as_bytes())
            .map_err(|_| invalid_input("name contains NUL"))?;
        let cto = std::ffi::CString::new(to.as_bytes())
            .map_err(|_| invalid_input("name contains NUL"))?;
        let rc = unsafe {
            libc::renameat(
                instance.dir.as_raw_fd(),
                cfrom.as_ptr(),
                instance.dir.as_raw_fd(),
                cto.as_ptr(),
            )
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        instance.dir.sync_all()?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        std::fs::rename(instance.path.join(from), instance.path.join(to))
    }
}

pub(super) fn unlink_relative(instance: &TrustedInstanceDir, name: &str) -> io::Result<()> {
    validate_single_component_name(name)?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStrExt;
        match fstatat_relative(instance, name) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
            Ok(meta) if meta.is_symlink => {
                return Err(invalid_input("refusing to unlink symlink"));
            }
            Ok(meta) if !meta.is_file => {
                return Err(invalid_input("expected regular file for removal"));
            }
            Ok(_) => {}
        }
        let cname = std::ffi::CString::new(name.as_bytes())
            .map_err(|_| invalid_input("name contains NUL"))?;
        let rc = unsafe { libc::unlinkat(instance.dir.as_raw_fd(), cname.as_ptr(), 0) };
        if rc != 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(err);
        }
        instance.dir.sync_all()?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        match std::fs::remove_file(instance.path.join(name)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

pub(super) fn regular_exists_relative(
    instance: &TrustedInstanceDir,
    name: &str,
) -> io::Result<bool> {
    validate_single_component_name(name)?;
    #[cfg(unix)]
    {
        match fstatat_relative(instance, name) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
            Ok(meta) if meta.is_symlink => Err(invalid_input("refusing symlink existence check")),
            Ok(meta) => Ok(meta.is_file),
        }
    }
    #[cfg(not(unix))]
    {
        Ok(instance.path.join(name).is_file())
    }
}

pub(super) fn content_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn file_hash_relative(
    instance: &TrustedInstanceDir,
    name: &str,
    max: u64,
) -> io::Result<Option<String>> {
    Ok(read_optional_regular_relative(instance, name, max)?.map(|b| content_hash(&b)))
}

/// Staged temp names are `.{final}.{pid}.{uuid_simple}.tmp` (single component).
pub(super) fn is_valid_staged_temp_name(tmp: &str, final_name: &str) -> bool {
    if validate_single_component_name(tmp).is_err() {
        return false;
    }
    if tmp.len() > MAX_TEMP_NAME_BYTES {
        return false;
    }
    let prefix = format!(".{final_name}.");
    if !tmp.starts_with(&prefix) || !tmp.ends_with(".tmp") {
        return false;
    }
    let mid = &tmp[prefix.len()..tmp.len() - ".tmp".len()];
    let Some((pid, uuid_part)) = mid.split_once('.') else {
        return false;
    };
    if pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if uuid_part.is_empty() || !uuid_part.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    true
}

/// Remove the instance directory via dirfds only (no path-based `remove_dir_all`).
pub(super) fn remove_instance_dir_locked(
    inst: &TrustedInstanceDir,
    instance_component: &str,
) -> io::Result<()> {
    validate_single_component_name(instance_component)?;
    for name in [
        CATALOG_FILE,
        CAPABILITIES_FILE,
        STATE_FILE,
        TXN_FILE,
        LOCK_FILE,
    ] {
        match unlink_relative_allow_missing(inst, name) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::InvalidInput => return Err(e),
            Err(e) => return Err(e),
        }
    }
    remove_orphan_temps(inst)?;
    rmdirat_instance(inst, instance_component)
}

fn unlink_relative_allow_missing(inst: &TrustedInstanceDir, name: &str) -> io::Result<()> {
    match unlink_relative(inst, name) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn remove_orphan_temps(inst: &TrustedInstanceDir) -> io::Result<()> {
    let Ok(rd) = std::fs::read_dir(&inst.path) else {
        return Ok(());
    };
    for ent in rd.flatten() {
        let name = ent.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let is_staged = [
            CATALOG_FILE,
            CAPABILITIES_FILE,
            STATE_FILE,
            TXN_FILE,
            LOCK_FILE,
        ]
        .iter()
        .any(|final_name| is_valid_staged_temp_name(name, final_name));
        if is_staged {
            let _ = unlink_relative(inst, name);
        }
    }
    Ok(())
}

fn rmdirat_instance(inst: &TrustedInstanceDir, instance_component: &str) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStrExt;
        let cname = std::ffi::CString::new(instance_component.as_bytes())
            .map_err(|_| invalid_input("instance name contains NUL"))?;
        let rc =
            unsafe { libc::unlinkat(inst.root.as_raw_fd(), cname.as_ptr(), libc::AT_REMOVEDIR) };
        if rc != 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(err);
        }
        let _ = inst.root.sync_all();
        Ok(())
    }
    #[cfg(not(unix))]
    {
        match std::fs::remove_dir(&inst.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// Bounded no-follow read of a regular file under `grok_home` (single component).
pub(super) fn read_home_regular_nofollow(
    grok_home: &Path,
    file_name: &str,
    max_bytes: u64,
) -> io::Result<Option<Vec<u8>>> {
    validate_single_component_name(file_name)?;
    let path = grok_home.join(file_name);
    let meta = match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
        Ok(m) => m,
    };
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(invalid_input("legacy cache is not a regular file"));
    }
    if meta.len() > max_bytes {
        return Err(invalid_data("legacy cache exceeds bound"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let uid = unsafe { libc::geteuid() };
        if meta.uid() != uid {
            return Err(invalid_input("legacy cache has unexpected owner"));
        }
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        let mut file = options.open(&path)?;
        let opened = file.metadata()?;
        if !opened.is_file() || opened.len() > max_bytes {
            return Err(invalid_data("legacy cache changed during open"));
        }
        let mut bytes = Vec::with_capacity(opened.len() as usize);
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > max_bytes {
            return Err(invalid_data("legacy cache exceeds bound"));
        }
        Ok(Some(bytes))
    }
    #[cfg(not(unix))]
    {
        let bytes = std::fs::read(&path)?;
        if bytes.len() as u64 > max_bytes {
            return Err(invalid_data("legacy cache exceeds bound"));
        }
        Ok(Some(bytes))
    }
}
