//! Distinct OpenRouter-native CLI namespace (`grok openrouter ...`).

use super::generated_ops::operations_for_namespace;
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
    /// Best-effort account key metadata (operation_id resolved from inventory).
    Key,
    /// Best-effort credits read.
    Credits,
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
            // Prefer inventory operation ids containing "key".
            if let Some(op) = operations_for_namespace("openrouter")
                .find(|op| op.operation_id.to_ascii_lowercase().contains("key") && op.method == "GET")
            {
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
                "hint": "use `grok openrouter ops` and `grok openrouter call <operationId>`",
                "provider": args.provider,
            }))
            .map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
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
}
