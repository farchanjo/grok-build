//! Static scan: active user-facing guidance must not advertise removed
//! global `/login`, `/logout`, `grok login`, or `grok logout`.
//!
//! Historical changelogs, protocol method ids (`x.ai/auth/logout`), and
//! deprecation error text that explicitly say "no longer supported" are
//! excluded via allowlist patterns.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("repo root")
    }

    fn collect_files(root: &Path, out: &mut Vec<PathBuf>) {
        if root.is_file() {
            out.push(root.to_path_buf());
            return;
        }
        let Ok(rd) = std::fs::read_dir(root) else {
            return;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || name == "target" {
                    continue;
                }
                collect_files(&p, out);
            } else {
                out.push(p);
            }
        }
    }

    fn is_excluded_path(path: &str) -> bool {
        let p = path.replace('\\', "/");
        p.contains("/changelogs/")
            || p.contains("CHANGELOG")
            || p.contains("/fixtures/")
            || p.contains("historic_")
            || p.ends_with(".jsonl")
            || p.contains("target-")
            || p.contains("/acp_session_tests/")
            || p.contains("/dispatch/tests/")
            || p.contains("/acp_handler/tests/")
            || p.contains("provider_guidance_scan.rs")
            || p.contains("pull_smoke_test.rs")
    }

    fn line_is_allowed(line: &str) -> bool {
        let l = line.trim();
        if l.starts_with("//") || l.starts_with("///") || l.starts_with('*') {
            return true; // comments not user-facing guidance
        }
        if l.contains("no longer supported")
            || l.contains("is no longer")
            || l.contains("there is no global")
            || l.contains("There is no global")
            || l.contains("must not")
            || l.contains("never mention")
            || l.contains("x.ai/auth/logout")
            || l.contains("serialize auth/logout")
            || l.contains("assert!")
            || l.contains("does **not** mention")
            || l.contains("does not mention")
            || l.contains("line.contains(")
        // scanner self-reference
        {
            return true;
        }
        false
    }

    #[test]
    fn active_user_facing_guidance_has_no_global_login_logout() {
        let root = repo_root();
        let mut offenders = Vec::new();

        let scan_roots = [
            root.join("crates/codegen/xai-grok-pager/src"),
            root.join("crates/codegen/xai-grok-pager/docs"),
            root.join("crates/codegen/xai-grok-pager-bin/src"),
            root.join("crates/codegen/xai-grok-shell/src"),
            root.join("crates/codegen/xai-grok-shell/README.md"),
            root.join("crates/codegen/xai-grok-voice/src"),
            root.join("crates/codegen/xai-grok-workspace/src/hub_auth.rs"),
        ];

        let mut files = Vec::new();
        for scan in &scan_roots {
            if scan.exists() {
                collect_files(scan, &mut files);
            }
        }

        for path in files {
            let path_str = path.to_string_lossy();
            if is_excluded_path(&path_str) {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "rs" | "md") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for (line_no, line) in text.lines().enumerate() {
                if line_is_allowed(line) {
                    continue;
                }
                let active_slash = line.contains("`/login`")
                    || line.contains("`/logout`")
                    || line.contains(" run /login")
                    || line.contains(" type /login")
                    || line.contains(" use /login");
                let active_cli = line.contains("`grok login`")
                    || line.contains("`grok logout`")
                    || line.contains("Run `grok login`")
                    || line.contains("run `grok login`")
                    || line.contains("run 'grok login'");
                if !(active_slash || active_cli) {
                    continue;
                }
                if line.contains("no longer supported") || line.contains("there is no global") {
                    continue;
                }
                offenders.push(format!("{}:{}: {}", path_str, line_no + 1, line.trim()));
            }
        }

        assert!(
            offenders.is_empty(),
            "active user-facing guidance still advertises global login/logout:\n{}",
            offenders.join("\n")
        );
    }
}
