//! Typed CLI dispatch entry (generated catalog + runtime).

use super::generated_ops::find_cli_operation;
use super::output::ExitCode;
use super::typed_dispatch_runtime;
use std::path::{Path, PathBuf};

/// Dispatch a typed operation. `input_json` must deserialize to the Params type.
pub async fn dispatch_typed_operation(
    provider: &str,
    namespace: &str,
    operation_id: &str,
    path_params: &[(String, String)],
    query: &[(String, String)],
    input_json: Option<&str>,
    dry_run: bool,
    stream: bool,
    output: Option<&Path>,
    multipart_files: &[(String, PathBuf)],
) -> Result<ExitCode, String> {
    let op = find_cli_operation(namespace, operation_id)
        .ok_or_else(|| format!("unknown operation {operation_id} in {namespace}"))?;
    if op.is_binary && output.is_none() {
        use std::io::IsTerminal;
        if std::io::stdout().is_terminal() {
            return Err("binary response requires --output <path> (refusing TTY)".into());
        }
    }
    if !matches!(
        op.method,
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD"
    ) {
        return Err(format!("unsupported HTTP method {}", op.method));
    }
    typed_dispatch_runtime::dispatch_runtime(
        provider,
        op,
        path_params,
        query,
        input_json,
        dry_run,
        stream,
        output,
        multipart_files,
    )
    .await
}
