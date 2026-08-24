//! Hermetic, filesystem-only Markdown link test for the PR22 documentation.
//!
//! This test never fetches a URL, never contacts the network, never runs a
//! binary, and never reads `GROK_HOME` or any external state. It validates
//! only that relative Markdown targets in the PR22-changed documentation
//! fixtures resolve to real files inside this repository, and that required
//! discovery/cross-links are present. It does not check external availability
//! and does not parse Rust source.

use std::path::{Component, Path, PathBuf};

// ---------------------------------------------------------------------------
// Fixture set: exactly the PR22 documentation slice. Kept explicit so the
// reviewable scope does not silently grow to unrelated Markdown.
// ---------------------------------------------------------------------------

const PR22_USER_GUIDES: &[&str] = &[
    "docs/user-guide/29-multi-account-providers.md",
    "docs/user-guide/30-retrieval-and-prime.md",
    "docs/user-guide/05-configuration.md",
    "docs/user-guide/08-skills.md",
    "docs/user-guide/09-plugins.md",
    "docs/user-guide/11-custom-models.md",
    "docs/user-guide/13-memory.md",
    "docs/user-guide/16-subagents.md",
    "docs/user-guide/31-strict-skills-migration.md",
    "docs/user-guide/README.md",
];

const PR22_PROVIDERS: &[&str] = &[
    "docs/providers/openai-platform.md",
    "docs/providers/openrouter.md",
];

fn pager_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    normalize(&pager_root().join("../../.."))
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn is_within(path: &Path, root: &Path) -> bool {
    path.strip_prefix(root).is_ok()
}

// ---------------------------------------------------------------------------
// Minimal Markdown link scanner (no parser crate, no external fetch).
// ---------------------------------------------------------------------------

/// True when `line` opens or closes a fenced code block.
fn is_fence_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    match trimmed.chars().next() {
        Some('`') => trimmed.chars().take_while(|&c| c == '`').count() >= 3,
        Some('~') => trimmed.chars().take_while(|&c| c == '~').count() >= 3,
        _ => false,
    }
}

/// Extract destination strings from a single (non-fenced) source line.
fn scan_line(line: &str, out: &mut Vec<String>) {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        // Link opener: `](`.
        if chars[i] == ']' && chars.get(i + 1) == Some(&'(') {
            // Distinguish `![alt](...)` images from documentation links.
            let is_image = {
                let mut j = i;
                let mut found_open = false;
                while j > 0 {
                    j -= 1;
                    if chars[j] == '[' {
                        found_open = true;
                        break;
                    }
                    if chars[j] == ']' {
                        break;
                    }
                }
                found_open && j >= 1 && chars[j - 1] == '!'
            };
            if !is_image {
                let start = i + 2;
                let dest = if chars.get(start) == Some(&'<') {
                    chars.get(start + 1..).and_then(|tail| {
                        tail.iter().position(|&c| c == '>').map(|pos| {
                            (start + 1..start + 1 + pos)
                                .map(|x| chars[x])
                                .collect::<String>()
                        })
                    })
                } else {
                    chars.get(start..).and_then(|tail| {
                        tail.iter()
                            .position(|&c| c == ')')
                            .map(|pos| (start..start + pos).map(|x| chars[x]).collect::<String>())
                    })
                };
                if let Some(dest) = dest {
                    push_local_target(dest, out);
                }
            }
            i += 2;
            continue;
        }
        i += 1;
    }
}

/// Classify a raw destination; push only local, resolvable relative targets.
fn push_local_target(raw: String, out: &mut Vec<String>) {
    let mut target = raw.trim().to_string();
    if let Some(hash) = target.find('#') {
        target.truncate(hash);
    }
    let target = target.trim().to_string();
    if target.is_empty() {
        return;
    }
    let lower = target.to_ascii_lowercase();
    // External schemes, mailto, protocol-relative links, bare anchors,
    // absolute paths, and any `scheme:`-style destination are not local.
    let external = lower.contains("://")
        || lower.starts_with("mailto:")
        || lower.starts_with("//")
        || lower.contains(':')
        || target.starts_with('#')
        || target.starts_with('/')
        || target.starts_with('\\');
    if external {
        return;
    }
    // A query string cannot be a valid local filename; skip it rather than
    // accidentally interpreting it as a filename.
    if target.contains('?') {
        return;
    }
    out.push(target);
}

/// Extract all local relative destinations in a Markdown body, ignoring
/// fenced code blocks so shell/TOML examples are not misread as links.
fn extract_local_targets(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in content.lines() {
        if is_fence_line(line) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        scan_line(line, &mut out);
    }
    out
}

// ---------------------------------------------------------------------------
// Main resolver test
// ---------------------------------------------------------------------------

#[test]
fn pr22_local_markdown_links_resolve_within_repository() {
    let pager = pager_root();
    let root = repo_root();
    assert!(
        root.is_dir(),
        "repo root must be a directory: {}",
        root.display()
    );
    assert!(
        root.join("README.md").is_file(),
        "repo root must contain README.md: {}",
        root.display()
    );

    // Assemble the PR22 fixture set (user-guide + providers + root README).
    let mut fixtures: Vec<PathBuf> = PR22_USER_GUIDES
        .iter()
        .map(|rel| normalize(&pager.join(rel)))
        .chain(PR22_PROVIDERS.iter().map(|rel| normalize(&pager.join(rel))))
        .collect();
    fixtures.push(root.join("README.md"));

    let mut failures: Vec<String> = Vec::new();
    for fixture in fixtures {
        let source = fixture.display().to_string();
        let content = std::fs::read_to_string(&fixture).unwrap_or_else(|e| {
            failures.push(format!("source={source} reason=unreadable error={e}"));
            String::new()
        });
        let src_dir = match fixture.parent() {
            Some(dir) => dir.to_path_buf(),
            None => {
                failures.push(format!("source={source} has no parent directory"));
                continue;
            }
        };
        let mut local_targets = extract_local_targets(&content);
        local_targets.sort();
        local_targets.dedup();
        for target in local_targets {
            let resolved = normalize(&src_dir.join(&target));
            if !is_within(&resolved, &root) {
                failures.push(format!(
                    "source={source} target={target} escaped the repository root"
                ));
            } else if !resolved.exists() {
                failures.push(format!(
                    "source={source} target={target} does not exist (resolved={})",
                    resolved.display()
                ));
            } else {
                // Require a regular file for `.md` targets. Directory links
                // already present in the root README are allowed to exist
                // without a file-ness check.
                if target.ends_with(".md") && !resolved.is_file() {
                    failures.push(format!(
                        "source={source} target={target} is not a regular file (resolved={})",
                        resolved.display()
                    ));
                }
                // Guard against a symlink that escapes the repository.
                if let Ok(canonical) = resolved.canonicalize() {
                    if let Ok(root_canon) = root.canonicalize() {
                        if !is_within(&canonical, &root_canon) {
                            failures.push(format!(
                                "source={source} target={target} escapes repository via symlink"
                            ));
                        }
                    }
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "PR22 local Markdown link failures:\n  {}",
        failures.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Required discovery / cross-link assertions (exact target strings only)
// ---------------------------------------------------------------------------

fn read_fixture(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read test fixture {}: {e}", path.display()))
}

fn user_guide(name: &str) -> String {
    read_fixture(&pager_root().join("docs/user-guide").join(name))
}

fn provider(name: &str) -> String {
    read_fixture(&pager_root().join("docs/providers").join(name))
}

/// Assert a raw target string is present as a `](target)` destination.
fn assert_links_to(content: &str, expected: &str, context: &str) {
    let marker = format!("]({expected})");
    assert!(
        content.contains(&marker),
        "{context} must link to {expected} (missing `{marker}`)"
    );
}

#[test]
fn pr22_required_discovery_and_cross_links_present() {
    let user_guide_readme = user_guide("README.md");
    let g29 = user_guide("29-multi-account-providers.md");
    let g30 = user_guide("30-retrieval-and-prime.md");
    let g05 = user_guide("05-configuration.md");
    let g11 = user_guide("11-custom-models.md");
    let g13 = user_guide("13-memory.md");
    let g16 = user_guide("16-subagents.md");
    let oai = provider("openai-platform.md");
    let or = provider("openrouter.md");
    let root_readme = read_fixture(&repo_root().join("README.md"));

    // user-guide index lists and links both new guides (exact filenames).
    assert_links_to(
        &user_guide_readme,
        "29-multi-account-providers.md",
        "user-guide README",
    );
    assert_links_to(
        &user_guide_readme,
        "30-retrieval-and-prime.md",
        "user-guide README",
    );

    // Root README carries repository-relative links to both guides and both
    // provider reference pages.
    for expected in [
        "crates/codegen/xai-grok-pager/docs/user-guide/29-multi-account-providers.md",
        "crates/codegen/xai-grok-pager/docs/user-guide/30-retrieval-and-prime.md",
        "crates/codegen/xai-grok-pager/docs/providers/openai-platform.md",
        "crates/codegen/xai-grok-pager/docs/providers/openrouter.md",
    ] {
        assert_links_to(&root_readme, expected, "root README");
    }

    // Guide 29 links to both provider reference pages and guide 30.
    assert_links_to(&g29, "../providers/openai-platform.md", "guide 29");
    assert_links_to(&g29, "../providers/openrouter.md", "guide 29");
    assert_links_to(&g29, "30-retrieval-and-prime.md", "guide 29");

    // Guide 30 links to both provider reference pages and guide 29.
    assert_links_to(&g30, "../providers/openai-platform.md", "guide 30");
    assert_links_to(&g30, "../providers/openrouter.md", "guide 30");
    assert_links_to(&g30, "29-multi-account-providers.md", "guide 30");

    // Configuration links to both new guides.
    assert_links_to(&g05, "29-multi-account-providers.md", "guide 05");
    assert_links_to(&g05, "30-retrieval-and-prime.md", "guide 05");

    // Focused guides link to the relevant perspectives.
    assert_links_to(&g11, "29-multi-account-providers.md", "guide 11");
    assert_links_to(&g11, "30-retrieval-and-prime.md", "guide 11");
    assert_links_to(&g13, "30-retrieval-and-prime.md", "guide 13");
    assert_links_to(&g13, "05-configuration.md", "guide 13");
    assert_links_to(&g16, "29-multi-account-providers.md", "guide 16");
    assert_links_to(&g16, "30-retrieval-and-prime.md", "guide 16");

    // Both provider pages link back to guide 29; OpenRouter also links to 30.
    assert_links_to(
        &oai,
        "../user-guide/29-multi-account-providers.md",
        "openai-platform",
    );
    assert_links_to(&oai, "../user-guide/05-configuration.md", "openai-platform");
    assert_links_to(
        &or,
        "../user-guide/29-multi-account-providers.md",
        "openrouter",
    );
    assert_links_to(&or, "../user-guide/30-retrieval-and-prime.md", "openrouter");
    assert_links_to(&or, "../user-guide/05-configuration.md", "openrouter");
}
