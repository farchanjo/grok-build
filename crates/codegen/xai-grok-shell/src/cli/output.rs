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
pub fn write_binary(bytes: &[u8], output: Option<&Path>) -> io::Result<ExitCode> {
    if let Some(path) = output {
        std::fs::write(path, bytes)?;
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
