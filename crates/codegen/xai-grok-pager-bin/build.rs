use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    emit_git_rerun_paths();
    println!("cargo:rerun-if-env-changed=GROK_VERSION");

    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let version = std::env::var("GROK_VERSION")
        .or_else(|_| std::env::var("CARGO_PKG_VERSION"))
        .unwrap_or_else(|_| "0.0.0".to_string());

    println!(
        "cargo:rustc-env=VERSION_WITH_COMMIT={} ({})",
        version, commit
    );
}

/// Emit `rerun-if-changed` for the git state that feeds `VERSION_WITH_COMMIT`.
///
/// Cargo resolves a relative watch path against the package root, so the
/// historical `.git/HEAD` never resolved: the `.git` directory lives three
/// levels above this crate. A missing watched file pins the build script dirty
/// on every build, so watch absolute paths instead.
///
/// The watched set includes the ref `HEAD` points at, because a commit rewrites
/// the ref rather than `HEAD` — without it the binary would report a stale
/// commit string after a commit that touched no file in this crate.
fn emit_git_rerun_paths() {
    let Some(repo_root) = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .as_deref()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
    else {
        println!("cargo:rerun-if-changed=.git/HEAD");
        return;
    };

    let dot_git = repo_root.join(".git");
    // Worktrees and submodules keep `.git` as a file pointing at the real dir.
    let git_dir = if dot_git.is_dir() {
        Some(dot_git.clone())
    } else if dot_git.is_file() {
        println!("cargo:rerun-if-changed={}", dot_git.display());
        std::fs::read_to_string(&dot_git)
            .ok()
            .and_then(|text| {
                text.strip_prefix("gitdir:")
                    .map(|p| PathBuf::from(p.trim()))
            })
            .map(|p| {
                if p.is_absolute() {
                    p
                } else {
                    repo_root.join(p)
                }
            })
    } else {
        None
    };

    let Some(git_dir) = git_dir else {
        println!("cargo:rerun-if-changed=.git/HEAD");
        eprintln!(
            "note: no git directory at {}; VERSION_WITH_COMMIT may be stale",
            dot_git.display()
        );
        return;
    };

    let head = git_dir.join("HEAD");
    if !head.exists() {
        println!("cargo:rerun-if-changed={}", git_dir.display());
        return;
    }
    println!("cargo:rerun-if-changed={}", head.display());

    if let Ok(text) = std::fs::read_to_string(&head)
        && let Some(reference) = text.trim().strip_prefix("ref:")
    {
        let ref_path = git_dir.join(reference.trim());
        if ref_path.exists() {
            println!("cargo:rerun-if-changed={}", ref_path.display());
        }
    }

    let packed_refs = git_dir.join("packed-refs");
    if packed_refs.exists() {
        println!("cargo:rerun-if-changed={}", packed_refs.display());
    }
}
