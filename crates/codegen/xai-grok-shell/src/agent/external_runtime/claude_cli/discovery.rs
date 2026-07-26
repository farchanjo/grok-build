//! Discover and probe the official `claude` executable.
//!
//! Precedence: explicit configured path first, then `PATH`. Never shell strings.
//! Canonicalize; require a regular executable file; reject relative/untrusted
//! invalid paths. Probe `claude --version` under timeout; strict semantic
//! version parse. Minimum version from fixture capabilities (preliminary
//! >= 2.1.217). Required capability checks win over version when present.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::process::{self, ProbeCommandResult};

/// Preliminary minimum Claude Code version for this integration.
///
/// Floor is driven by protocol flags we require (`--safe-mode`,
/// `--forward-subagent-text`, budget cap enforcement, `capabilities` on
/// `system/init`). Raise only with protocol evidence.
pub const MIN_CLAUDE_CLI_VERSION: &str = "2.1.217";

/// Default probe timeout for `--version` / capability probes.
pub const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum captured stdout/stderr for a version probe.
pub const VERSION_PROBE_OUTPUT_CAP: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCliDiscovery {
    pub executable: PathBuf,
    pub version: semver::Version,
    pub capabilities: Vec<String>,
    /// File length at discovery time (replacement detection).
    pub file_len: u64,
    /// mtime at discovery time when available.
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeCliDiscoveryError {
    NotFound {
        detail: String,
    },
    InvalidPath {
        path: String,
        detail: String,
    },
    NotExecutable {
        path: String,
    },
    ProbeTimeout {
        path: String,
    },
    ProbeFailed {
        path: String,
        detail: String,
    },
    VersionParse {
        path: String,
        raw: String,
    },
    VersionTooOld {
        path: String,
        found: String,
        minimum: String,
    },
    MissingCapability {
        path: String,
        capability: String,
        version: String,
    },
}

impl std::fmt::Display for ClaudeCliDiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { detail } => {
                write!(
                    f,
                    "Claude Agent CLI not found ({detail}). Install the official `claude` binary \
                     or set an absolute path via GROK_CLAUDE_CLI_PATH."
                )
            }
            Self::InvalidPath { path, detail } => {
                write!(f, "Claude Agent CLI path '{path}' is invalid: {detail}")
            }
            Self::NotExecutable { path } => {
                write!(
                    f,
                    "Claude Agent CLI path '{path}' is not a regular executable file"
                )
            }
            Self::ProbeTimeout { path } => {
                write!(
                    f,
                    "Claude Agent CLI version probe timed out for '{path}' (hung or unresponsive)"
                )
            }
            Self::ProbeFailed { path, detail } => {
                write!(
                    f,
                    "Claude Agent CLI version probe failed for '{path}': {detail}"
                )
            }
            Self::VersionParse { path, raw } => {
                write!(
                    f,
                    "Claude Agent CLI at '{path}' returned an unparseable version: {raw:?}"
                )
            }
            Self::VersionTooOld {
                path,
                found,
                minimum,
            } => {
                write!(
                    f,
                    "Claude Agent CLI at '{path}' is {found}; this integration requires >= {minimum}"
                )
            }
            Self::MissingCapability {
                path,
                capability,
                version,
            } => {
                write!(
                    f,
                    "Claude Agent CLI at '{path}' (v{version}) is missing required capability '{capability}'"
                )
            }
        }
    }
}

impl std::error::Error for ClaudeCliDiscoveryError {}

/// Result of a successful discovery + version probe (capabilities optional).
pub type ClaudeCliProbeResult = Result<ClaudeCliDiscovery, ClaudeCliDiscoveryError>;

/// Environment override for an absolute configured executable path.
pub const CLAUDE_CLI_PATH_ENV: &str = "GROK_CLAUDE_CLI_PATH";

/// Discover the official executable.
///
/// `configured_path` wins when present; otherwise `PATH` is searched for
/// `claude`. Never constructs a shell string.
pub fn discover_claude_executable(
    configured_path: Option<&Path>,
) -> Result<PathBuf, ClaudeCliDiscoveryError> {
    if let Some(p) = configured_path {
        return validate_executable_path(p);
    }
    if let Ok(env_path) = std::env::var(CLAUDE_CLI_PATH_ENV) {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            return validate_executable_path(Path::new(trimmed));
        }
    }
    find_on_path("claude").ok_or_else(|| ClaudeCliDiscoveryError::NotFound {
        detail: "no configured path and `claude` not on PATH".into(),
    })
}

/// Validate a candidate path: absolute after canonicalize, regular file,
/// executable bit (Unix). Relative paths are rejected before canonicalize
/// when they are clearly relative and non-existent after join? Spec: reject
/// relative/untrusted invalid path — we require absolute input or resolve
/// via PATH only (PATH entries produce absolute via canonicalize).
pub fn validate_executable_path(path: &Path) -> Result<PathBuf, ClaudeCliDiscoveryError> {
    let display = path.display().to_string();
    if path.as_os_str().is_empty() {
        return Err(ClaudeCliDiscoveryError::InvalidPath {
            path: display,
            detail: "empty path".into(),
        });
    }
    // Reject relative configured paths (PATH search is the only relative name
    // entry point, and it resolves directory joins to absolute before validate).
    if path.is_relative() {
        return Err(ClaudeCliDiscoveryError::InvalidPath {
            path: display,
            detail: "relative paths are not allowed; use an absolute path or PATH discovery".into(),
        });
    }
    if !path.exists() {
        return Err(ClaudeCliDiscoveryError::InvalidPath {
            path: display,
            detail: "path does not exist".into(),
        });
    }
    let meta = std::fs::metadata(path).map_err(|e| ClaudeCliDiscoveryError::InvalidPath {
        path: display.clone(),
        detail: format!("metadata: {e}"),
    })?;
    if !meta.is_file() {
        return Err(ClaudeCliDiscoveryError::NotExecutable { path: display });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o111 == 0 {
            return Err(ClaudeCliDiscoveryError::NotExecutable { path: display });
        }
    }
    let canonical =
        std::fs::canonicalize(path).map_err(|e| ClaudeCliDiscoveryError::InvalidPath {
            path: display.clone(),
            detail: format!("canonicalize: {e}"),
        })?;
    if !canonical.is_absolute() {
        return Err(ClaudeCliDiscoveryError::InvalidPath {
            path: display,
            detail: "canonical path is not absolute".into(),
        });
    }
    Ok(canonical)
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let candidate = dir.join(name);
        if candidate.is_file() {
            // PATH entries may be relative dirs; only accept if canonicalize
            // yields a regular executable.
            if let Ok(abs) = std::fs::canonicalize(&candidate) {
                if validate_executable_path(&abs).is_ok() {
                    return Some(abs);
                }
            }
        }
    }
    None
}

/// Strict semantic version extraction from `claude --version` output.
///
/// Accepts forms like `2.1.217`, `claude 2.1.217`, `Claude Code 2.1.217 (foo)`.
pub fn parse_claude_version(raw: &str) -> Option<semver::Version> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Prefer the first X.Y.Z token.
    let re = regex_lite_semver_token(trimmed)?;
    semver::Version::parse(&re).ok()
}

/// Minimal token finder without pulling a regex crate solely for this.
fn regex_lite_semver_token(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut dots = 0;
            let mut j = i;
            while j < bytes.len() {
                let c = bytes[j];
                if c.is_ascii_digit() {
                    j += 1;
                } else if c == b'.' {
                    dots += 1;
                    j += 1;
                    if j >= bytes.len() || !bytes[j].is_ascii_digit() {
                        break;
                    }
                } else {
                    break;
                }
            }
            if dots >= 2 {
                let token = &s[start..j];
                // Strip trailing pre-release separators for core version.
                let core: String = token
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                // Need major.minor.patch at least.
                if core.split('.').count() >= 3 {
                    return Some(core);
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

/// Required capabilities when the protocol reports a `capabilities` array.
///
/// Empty for MVP: version floor is the primary gate; unknown optional
/// capabilities are forward-compatible (ignored). Callers may extend this
/// list when a capability becomes mandatory.
pub const REQUIRED_CAPABILITIES: &[&str] = &[];

/// Check required capabilities (when reported). Unknown caps are ignored.
pub fn check_required_capabilities(reported: &[String], required: &[&str]) -> Result<(), String> {
    if required.is_empty() {
        return Ok(());
    }
    // If the runtime reported no capabilities array content, do not fail on
    // required checks — fall back to version (caller responsibility). When
    // the array is present (even empty), enforce required membership.
    for req in required {
        if !reported.iter().any(|c| c == req) {
            return Err((*req).to_owned());
        }
    }
    Ok(())
}

/// Probe `claude --version` with timeout and validate minimum version.
pub async fn probe_claude_version(executable: &Path, timeout: Duration) -> ClaudeCliProbeResult {
    let args = ["--version"];
    let result = process::run_probe_command(executable, &args, timeout, VERSION_PROBE_OUTPUT_CAP)
        .await
        .map_err(|e| match e {
            process::ProbeError::Timeout => ClaudeCliDiscoveryError::ProbeTimeout {
                path: executable.display().to_string(),
            },
            process::ProbeError::Spawn(msg)
            | process::ProbeError::Io(msg)
            | process::ProbeError::ProcessGroup(msg) => ClaudeCliDiscoveryError::ProbeFailed {
                path: executable.display().to_string(),
                detail: msg,
            },
        })?;

    let ProbeCommandResult {
        stdout,
        stderr,
        success,
        ..
    } = result;

    if !success && stdout.trim().is_empty() {
        return Err(ClaudeCliDiscoveryError::ProbeFailed {
            path: executable.display().to_string(),
            detail: if stderr.trim().is_empty() {
                "non-zero exit with empty output".into()
            } else {
                truncate_for_status(&stderr, 256)
            },
        });
    }

    let combined = if stdout.trim().is_empty() {
        stderr.clone()
    } else {
        stdout.clone()
    };
    let version =
        parse_claude_version(&combined).ok_or_else(|| ClaudeCliDiscoveryError::VersionParse {
            path: executable.display().to_string(),
            raw: truncate_for_status(&combined, 128),
        })?;

    let minimum = semver::Version::parse(MIN_CLAUDE_CLI_VERSION).expect("static min version");
    if version < minimum {
        return Err(ClaudeCliDiscoveryError::VersionTooOld {
            path: executable.display().to_string(),
            found: version.to_string(),
            minimum: minimum.to_string(),
        });
    }

    let (file_len, modified) = file_identity(executable)?;
    Ok(ClaudeCliDiscovery {
        executable: executable.to_path_buf(),
        version,
        capabilities: Vec::new(),
        file_len,
        modified,
    })
}

/// Snapshot file identity for replacement detection.
pub fn file_identity(path: &Path) -> Result<(u64, Option<SystemTime>), ClaudeCliDiscoveryError> {
    let meta = std::fs::metadata(path).map_err(|e| ClaudeCliDiscoveryError::InvalidPath {
        path: path.display().to_string(),
        detail: format!("metadata: {e}"),
    })?;
    Ok((meta.len(), meta.modified().ok()))
}

/// Re-validate that `path` is still a regular executable and matches the
/// identity recorded at discovery (len + mtime). Fails closed on replacement
/// or permission loss.
pub fn revalidate_executable(
    path: &Path,
    expected: &ClaudeCliDiscovery,
) -> Result<PathBuf, ClaudeCliDiscoveryError> {
    let canonical = validate_executable_path(path)?;
    if canonical != expected.executable {
        return Err(ClaudeCliDiscoveryError::InvalidPath {
            path: path.display().to_string(),
            detail: "canonical path changed since discovery".into(),
        });
    }
    let (len, modified) = file_identity(&canonical)?;
    if len != expected.file_len {
        return Err(ClaudeCliDiscoveryError::InvalidPath {
            path: canonical.display().to_string(),
            detail: "executable was replaced (size changed)".into(),
        });
    }
    if expected.modified.is_some() && modified != expected.modified {
        return Err(ClaudeCliDiscoveryError::InvalidPath {
            path: canonical.display().to_string(),
            detail: "executable was replaced (mtime changed)".into(),
        });
    }
    Ok(canonical)
}

/// Full discovery: path resolve + version probe + required capability check.
pub async fn discover_and_probe(
    configured_path: Option<&Path>,
    reported_capabilities: Option<&[String]>,
) -> ClaudeCliProbeResult {
    let executable = discover_claude_executable(configured_path)?;
    let mut discovery = probe_claude_version(&executable, VERSION_PROBE_TIMEOUT).await?;
    if let Some(caps) = reported_capabilities {
        discovery.capabilities = caps.to_vec();
        if let Err(missing) = check_required_capabilities(caps, REQUIRED_CAPABILITIES) {
            return Err(ClaudeCliDiscoveryError::MissingCapability {
                path: executable.display().to_string(),
                capability: missing,
                version: discovery.version.to_string(),
            });
        }
    }
    Ok(discovery)
}

fn truncate_for_status(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.len() <= max {
        t.to_owned()
    } else {
        format!("{}…", &t[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_semver() {
        let v = parse_claude_version("2.1.217").unwrap();
        assert_eq!(v.to_string(), "2.1.217");
    }

    #[test]
    fn parses_prefixed_version_line() {
        let v = parse_claude_version("Claude Code 2.1.250 (stable)").unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 250);
    }

    #[test]
    fn rejects_empty_and_garbage() {
        assert!(parse_claude_version("").is_none());
        assert!(parse_claude_version("not a version").is_none());
        assert!(parse_claude_version("2.1").is_none());
    }

    #[test]
    fn rejects_relative_configured_path() {
        let err = validate_executable_path(Path::new("claude")).unwrap_err();
        assert!(matches!(err, ClaudeCliDiscoveryError::InvalidPath { .. }));
    }

    #[test]
    fn required_caps_empty_ok() {
        check_required_capabilities(&["interrupt_receipt_v1".into()], &[]).unwrap();
    }

    #[test]
    fn required_caps_enforced_when_listed() {
        let err =
            check_required_capabilities(&["other".into()], &["interrupt_receipt_v1"]).unwrap_err();
        assert_eq!(err, "interrupt_receipt_v1");
    }

    #[test]
    fn min_version_parses() {
        assert!(semver::Version::parse(MIN_CLAUDE_CLI_VERSION).is_ok());
    }
}
