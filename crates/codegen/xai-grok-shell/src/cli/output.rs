//! CLI output policy: JSON / NDJSON / binary TTY refusal / exit codes.

use serde::Serialize;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Ndjson,
    Human,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Success = 0,
    Usage = 2,
    Runtime = 1,
    Auth = 3,
    NotFound = 4,
    Cancelled = 130,
}

impl ExitCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Write a JSON value to stdout (stable default).
pub fn write_json<T: Serialize>(value: &T) -> io::Result<()> {
    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value)?;
    out.write_all(b"\n")?;
    out.flush()
}

/// Write one NDJSON line (streaming).
pub fn write_ndjson_line<T: Serialize>(value: &T) -> io::Result<()> {
    let mut out = io::stdout().lock();
    serde_json::to_writer(&mut out, value)?;
    out.write_all(b"\n")?;
    out.flush()
}

/// Refuse binary output to an interactive TTY unless `--output` is set.
///
/// File sinks use the same owner-only durable primitive as transport
/// `execute_binary` (`write_owner_only_atomic`): unique temp, mode 0600,
/// flush/sync, atomic rename, parent sync, symlink refusal, Drop cleanup.
pub fn write_binary(bytes: &[u8], output: Option<&Path>) -> io::Result<ExitCode> {
    if let Some(path) = output {
        xai_grok_inference::openai_platform::write_owner_only_atomic(path, bytes)?;
        return Ok(ExitCode::Success);
    }
    if io::stdout().is_terminal() {
        eprintln!(
            "error: refusing to write binary data to an interactive terminal; pass --output <path>"
        );
        return Ok(ExitCode::Usage);
    }
    let mut out = io::stdout().lock();
    out.write_all(bytes)?;
    out.flush()?;
    Ok(ExitCode::Success)
}

/// Read `--input` from a file path or `-` (stdin) and deserialize as typed `T`.
pub fn read_typed_input<T: serde::de::DeserializeOwned>(input: &str) -> Result<T, String> {
    let raw = if input == "-" {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("read stdin: {e}"))?;
        buf
    } else {
        std::fs::read_to_string(input).map_err(|e| format!("read {input}: {e}"))?
    };
    if raw.trim().is_empty() {
        return Err("input is empty".into());
    }
    serde_json::from_str(&raw).map_err(|e| format!("typed request deserialize failed: {e}"))
}

use std::io::Read;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "grok-cli-output-{}-{}-{}",
            label,
            std::process::id(),
            nanos
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn write_binary_file_is_owner_only_durable() {
        let dir = test_dir("bin");
        let path = dir.join("speech.bin");
        let code = write_binary(b"audio-bytes", Some(&path)).unwrap();
        assert_eq!(code, ExitCode::Success);
        assert_eq!(fs::read(&path).unwrap(), b"audio-bytes");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn write_binary_refuses_symlink_target() {
        let dir = test_dir("symlink");
        let real = dir.join("real.bin");
        fs::write(&real, b"keep").unwrap();
        let link = dir.join("link.bin");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&real, &link).unwrap();
            let err = write_binary(b"overwrite", Some(&link)).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
            assert_eq!(fs::read(&real).unwrap(), b"keep");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn exit_code_values_are_stable() {
        assert_eq!(ExitCode::Success.as_i32(), 0);
        assert_eq!(ExitCode::Runtime.as_i32(), 1);
        assert_eq!(ExitCode::Usage.as_i32(), 2);
        assert_eq!(ExitCode::Auth.as_i32(), 3);
        assert_eq!(ExitCode::NotFound.as_i32(), 4);
        assert_eq!(ExitCode::Cancelled.as_i32(), 130);
    }
}
