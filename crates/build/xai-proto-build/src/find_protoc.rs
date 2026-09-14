use anyhow::{Context, bail};
use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

fn check_protoc_good(protoc: &Path) -> anyhow::Result<()> {
    let output = Command::new(protoc)
        .arg("--version")
        .output()
        .context("Failed to execute protoc")?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "protoc --version failed, likely dotslash is missing; \
             try `cargo install dotslash`; stdout: {stdout:?}, stderr: {stderr:?}"
        );
    }
    Ok(())
}

fn is_github_actions() -> bool {
    env::var_os("GITHUB_ACTIONS").is_some()
}

/// Resolve an executable name against a `PATH`-style value.
///
/// Returns the first existing candidate as an **absolute** path. The caller
/// emits the result as `cargo:rerun-if-changed`, and cargo resolves a relative
/// watch path against the package root (the build script's cwd) — where a bare
/// `protoc` from `$PATH` does not exist. Returning the absolute path keeps the
/// build script fresh instead of dirty on every run.
///
/// `anchor` resolves relative `PATH` entries (legal, if rare); callers pass the
/// current directory.
fn find_on_path(name: &str, path_var: Option<&OsStr>, anchor: &Path) -> Option<PathBuf> {
    let path_var = path_var?;
    // Accept both the bare name and the platform-suffixed one so callers can
    // look up non-executable files in tests without special-casing Windows.
    let names = [
        name.to_string(),
        format!("{name}{}", env::consts::EXE_SUFFIX),
    ];

    for dir in env::split_paths(path_var) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        for candidate_name in &names {
            let candidate = dir.join(candidate_name);
            if !candidate.is_file() {
                continue;
            }
            return Some(if candidate.is_absolute() {
                candidate
            } else {
                anchor.join(candidate)
            });
        }
    }
    None
}

/// Find `protoc` command.
///
/// Search order:
/// 1. `$PROTOC` environment variable (set by Bazel `build_script_env` or user override)
/// 2. `bin/protoc` walking up parent directories (dotslash wrapper for local dev)
/// 3. `protoc` on `$PATH` (system install or other tooling)
///
/// When `bin/protoc` exists but fails to execute (e.g. the dotslash wrapper running
/// in Bazel remote execution where `dotslash` is not installed), the error is not fatal —
/// we fall through to the PATH-based lookup instead.
///
/// Returns `Ok(None)` if not found and not in a strict environment (GitHub Actions).
pub fn find_protoc() -> anyhow::Result<Option<PathBuf>> {
    // 1. Check the PROTOC env var first. This is the standard override used by prost-build
    //    and is set by Bazel cargo_build_script build_script_env to point at a hermetic
    //    protoc binary instead of the dotslash wrapper.
    if let Ok(protoc_env) = env::var("PROTOC") {
        let protoc = PathBuf::from(&protoc_env);
        if protoc.try_exists()? {
            check_protoc_good(&protoc)?;
            return Ok(Some(protoc));
        }
    }

    // 2. Walk up directories looking for bin/protoc (dotslash wrapper).
    let cwd = env::current_dir()?;
    let mut dir = cwd.clone();
    let mut dir_rel = PathBuf::new();
    loop {
        // Return relative path to make build more deterministic.
        let protoc = dir_rel.join("bin/protoc");
        if protoc.try_exists()? {
            match check_protoc_good(&protoc) {
                Ok(()) => return Ok(Some(protoc)),
                Err(e) => {
                    // bin/protoc exists but can't execute — likely the dotslash wrapper
                    // in an environment without dotslash (e.g. Bazel remote execution).
                    // Fall through to PATH-based lookup below.
                    eprintln!(
                        "bin/protoc found at `{}` but failed to execute: {e:#}; \
                         trying protoc from PATH as fallback",
                        protoc.display()
                    );
                    break;
                }
            }
        }
        if !dir.pop() {
            break;
        }
        dir_rel.push("..");
    }

    // 3. Try protoc from PATH (system install or other tooling). Returned as an
    //    absolute path; see `find_on_path` for why that matters.
    if let Some(protoc) = find_on_path(
        "protoc",
        env::var_os("PATH").as_deref(),
        &env::current_dir()?,
    ) && check_protoc_good(&protoc).is_ok()
    {
        return Ok(Some(protoc));
    }

    // 4. Not found anywhere.
    if is_github_actions() {
        return Err(anyhow::anyhow!(
            "`protoc` not found (checked $PROTOC env, bin/protoc, and PATH)"
        ));
    }
    eprintln!("`protoc` not found; likely it is missing in docker image");
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A dir holding a stub `protoc`, so tests never depend on the host's real
    /// protoc install or on the repository layout.
    fn dir_with_protoc() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().expect("temp dir");
        fs::write(
            dir.path()
                .join(format!("protoc{}", env::consts::EXE_SUFFIX)),
            b"stub",
        )
        .expect("write stub");
        dir
    }

    fn path_var(dirs: &[&Path]) -> std::ffi::OsString {
        env::join_paths(dirs.iter().map(|d| d.as_os_str())).expect("join PATH")
    }

    #[test]
    fn find_on_path_returns_absolute_path() {
        let dir = dir_with_protoc();
        let var = path_var(&[dir.path()]);
        let anchor = env::current_dir().expect("cwd");

        let found = find_on_path("protoc", Some(&var), &anchor).expect("protoc found on PATH");

        assert!(found.is_absolute(), "got non-absolute path: {found:?}");
        assert!(found.is_file(), "found path does not exist: {found:?}");
    }

    #[test]
    fn find_on_path_skips_dirs_without_the_executable() {
        let empty = tempfile::TempDir::new().expect("temp dir");
        let with = dir_with_protoc();
        let var = path_var(&[empty.path(), with.path()]);
        let anchor = env::current_dir().expect("cwd");

        let found = find_on_path("protoc", Some(&var), &anchor).expect("found in second entry");

        assert!(found.starts_with(with.path()), "got {found:?}");
    }

    #[test]
    fn find_on_path_returns_none_when_absent() {
        let empty = tempfile::TempDir::new().expect("temp dir");
        let var = path_var(&[empty.path()]);
        let anchor = env::current_dir().expect("cwd");

        assert!(find_on_path("protoc", Some(&var), &anchor).is_none());
    }

    #[test]
    fn find_on_path_returns_none_without_path_var() {
        let anchor = env::current_dir().expect("cwd");

        assert!(find_on_path("protoc", None, &anchor).is_none());
    }

    #[test]
    fn find_on_path_ignores_empty_entries() {
        let with = dir_with_protoc();
        let mut var = std::ffi::OsString::from("");
        var.push(env::join_paths([with.path()]).expect("join PATH"));
        let anchor = env::current_dir().expect("cwd");

        let found = find_on_path("protoc", Some(&var), &anchor).expect("found after empty entry");

        assert!(found.is_absolute(), "got non-absolute path: {found:?}");
    }

    #[test]
    fn find_on_path_anchors_relative_entries_to_anchor() {
        // `src/lib.rs` exists next to this file; using it avoids creating
        // anything on disk. The test binary's cwd is the package root.
        let anchor = env::current_dir().expect("cwd");

        let found = find_on_path("lib.rs", Some(OsStr::new("src")), &anchor)
            .expect("lib.rs found via relative entry");

        assert_eq!(found, anchor.join("src").join("lib.rs"));
        assert!(found.is_absolute(), "got non-absolute path: {found:?}");
    }
}
