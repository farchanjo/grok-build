//! Distinct OpenRouter-native CLI namespace (`grok openrouter ...`).

use super::generated_ops::{find_cli_operation, operations_for_namespace};
use super::output::{ExitCode, write_json};
use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(
    name = "openrouter",
    about = "OpenRouter-native operations (not OpenAI administration)"
)]
pub struct OpenRouterCliArgs {
    #[arg(long, default_value = "openrouter")]
    pub provider: String,

    #[arg(long)]
    pub dry_run: bool,

    #[arg(long)]
    pub yes: bool,

    #[command(subcommand)]
    pub command: OpenRouterCliCommand,
}

#[derive(Debug, Subcommand)]
pub enum OpenRouterCliCommand {
    /// List OpenRouter-native operations from the pinned baseline.
    Ops,
    /// Invoke an OpenRouter operation_id with typed --input.
    Call {
        operation_id: String,
        #[arg(long = "path-param", value_name = "NAME=VALUE")]
        path_params: Vec<String>,
        #[arg(long = "query", value_name = "NAME=VALUE")]
        query: Vec<String>,
        #[arg(long)]
        input: Option<String>,
    },
    /// Current application key metadata (`getCurrentKey` / exact `GET /key`).
    Key,
    /// Best-effort credits read.
    Credits,
}

/// Resolve the Key shorthand to the application `getCurrentKey` operation only.
///
/// Never selects BYOK/admin key-list operations (substring `"key"` is unsafe).
pub fn resolve_key_operation_id() -> Result<&'static str, String> {
    let op = find_cli_operation("openrouter", "getCurrentKey").ok_or_else(|| {
        "openrouter getCurrentKey binding missing from generated catalog".to_owned()
    })?;
    if op.path != "/key" || op.method != "GET" {
        return Err(format!(
            "getCurrentKey must be GET /key (got {} {})",
            op.method, op.path
        ));
    }
    if op.is_admin || op.credential_class != "application" {
        return Err("getCurrentKey must use application credential class (not admin/BYOK)".into());
    }
    Ok(op.operation_id)
}

pub async fn run_openrouter_cli(args: OpenRouterCliArgs) -> i32 {
    match run_inner(args).await {
        Ok(c) => c.as_i32(),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::Runtime.as_i32()
        }
    }
}

async fn run_inner(args: OpenRouterCliArgs) -> Result<ExitCode, String> {
    match args.command {
        OpenRouterCliCommand::Ops => {
            let ops: Vec<_> = operations_for_namespace("openrouter")
                .map(|op| {
                    json!({
                        "operation_id": op.operation_id,
                        "method": op.method,
                        "path": op.path,
                        "cli_route": op.cli_route,
                    })
                })
                .collect();
            write_json(&json!({
                "namespace": "openrouter",
                "note": "OpenRouter-native API; not OpenAI administration",
                "count": ops.len(),
                "operations": ops,
            }))
            .map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }
        OpenRouterCliCommand::Call {
            operation_id,
            path_params,
            query,
            input,
        } => {
            crate::cli::openai_cmd::call_namespace(
                &args.provider,
                "openrouter",
                &operation_id,
                &path_params,
                &query,
                input.as_deref(),
                args.dry_run,
                args.yes,
            )
            .await
        }
        OpenRouterCliCommand::Key => {
            let operation_id = resolve_key_operation_id()?;
            crate::cli::openai_cmd::call_namespace(
                &args.provider,
                "openrouter",
                operation_id,
                &[],
                &[],
                None,
                args.dry_run,
                args.yes,
            )
            .await
        }
        OpenRouterCliCommand::Credits => {
            if let Some(op) = operations_for_namespace("openrouter").find(|op| {
                op.operation_id.to_ascii_lowercase().contains("credit") && op.method == "GET"
            }) {
                return crate::cli::openai_cmd::call_namespace(
                    &args.provider,
                    "openrouter",
                    op.operation_id,
                    &[],
                    &[],
                    None,
                    args.dry_run,
                    args.yes,
                )
                .await;
            }
            write_json(&json!({
                "hint": "use `grok openrouter ops` for exact operation_id",
            }))
            .map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_ops() {
        let args = OpenRouterCliArgs::try_parse_from(["openrouter", "ops"]).unwrap();
        assert!(matches!(args.command, OpenRouterCliCommand::Ops));
    }

    #[test]
    fn key_shorthand_binds_get_current_key_application_not_byok() {
        let id = resolve_key_operation_id().expect("getCurrentKey");
        assert_eq!(id, "getCurrentKey");
        let op = find_cli_operation("openrouter", id).unwrap();
        assert_eq!(op.path, "/key");
        assert_eq!(op.method, "GET");
        assert!(!op.is_admin);
        assert_eq!(op.credential_class, "application");
        // Catalog order would hit listBYOKKeys first under substring "key".
        let first_keyish = operations_for_namespace("openrouter")
            .find(|op| op.operation_id.to_ascii_lowercase().contains("key") && op.method == "GET")
            .expect("keyish");
        assert_ne!(
            first_keyish.operation_id, "getCurrentKey",
            "fixture assumes substring order still prefers BYOK so this regression stays meaningful"
        );
        assert_ne!(first_keyish.path, "/key");
        assert!(first_keyish.is_admin || first_keyish.credential_class == "admin");
    }
}
