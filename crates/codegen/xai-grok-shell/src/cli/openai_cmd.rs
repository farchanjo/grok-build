//! `grok openai --provider <id> ...` typed platform CLI.
//!
//! All live calls go through the typed client via
//! [`super::generated_dispatch::dispatch_typed_operation`]. There is no
//! generic `Value` / `HttpRequestSpec` bypass path for supported operations.

use super::generated_dispatch::dispatch_typed_operation;
use super::generated_ops::{CLI_OPERATIONS, find_cli_operation, operations_for_namespace};
use super::output::{ExitCode, read_typed_input, write_json};
use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "openai",
    about = "Typed OpenAI platform operations against a configured provider"
)]
pub struct OpenAiCliArgs {
    /// Provider id (built-in `openai` or any configured OpenAI-compatible id).
    #[arg(long)]
    pub provider: String,

    /// Use administration credential and admin base URL.
    #[arg(long)]
    pub admin: bool,

    /// Dry-run shared-state mutations (print typed request only).
    #[arg(long)]
    pub dry_run: bool,

    /// Confirm shared-state mutations non-interactively.
    #[arg(long)]
    pub yes: bool,

    /// Write binary responses to this path (required for binary ops on TTY).
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Emit NDJSON for streaming / SSE operations.
    #[arg(long)]
    pub stream: bool,

    /// Multipart file field bindings as `field=/path/to/file` (repeatable).
    #[arg(long = "file", value_name = "FIELD=PATH")]
    pub files: Vec<String>,

    #[command(subcommand)]
    pub command: OpenAiCliCommand,
}

#[derive(Debug, Subcommand)]
pub enum OpenAiCliCommand {
    /// List all typed operations available under this CLI.
    Ops {
        #[arg(long)]
        json: bool,
    },
    /// Invoke a baseline operation by OpenAPI operation_id.
    ///
    /// Complex bodies use `--input <file|->` and deserialize into the
    /// operation's typed Params request (not raw JSON forwarding).
    Call {
        /// OpenAPI `operationId` (e.g. `listModels`, `createChatCompletion`).
        operation_id: String,
        /// Path parameters as `name=value` pairs.
        #[arg(long = "path-param", value_name = "NAME=VALUE")]
        path_params: Vec<String>,
        /// Query parameters as `name=value` pairs.
        #[arg(long = "query", value_name = "NAME=VALUE")]
        query: Vec<String>,
        /// Typed JSON request body file or `-` for stdin.
        #[arg(long)]
        input: Option<String>,
    },
    Models {
        #[command(subcommand)]
        command: ModelsCommand,
    },
    Chat {
        #[command(subcommand)]
        command: ChatCommand,
    },
    Responses {
        #[command(subcommand)]
        command: ResponsesCommand,
    },
    Embeddings {
        #[command(subcommand)]
        command: EmbeddingsCommand,
    },
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ModelsCommand {
    List {
        #[arg(long)]
        input: Option<String>,
    },
    Retrieve {
        model_id: String,
    },
    Delete {
        model_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ChatCommand {
    Create {
        #[arg(long)]
        input: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ResponsesCommand {
    Create {
        #[arg(long)]
        input: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum EmbeddingsCommand {
    Create {
        #[arg(long)]
        input: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum AdminCommand {
    Ops,
    Call {
        operation_id: String,
        #[arg(long = "path-param", value_name = "NAME=VALUE")]
        path_params: Vec<String>,
        #[arg(long = "query", value_name = "NAME=VALUE")]
        query: Vec<String>,
        #[arg(long)]
        input: Option<String>,
    },
}

pub async fn run_openai_cli(args: OpenAiCliArgs) -> i32 {
    match run_inner(args).await {
        Ok(code) => code.as_i32(),
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::Runtime.as_i32()
        }
    }
}

async fn run_inner(args: OpenAiCliArgs) -> Result<ExitCode, String> {
    let files = parse_files(&args.files)?;
    match args.command {
        OpenAiCliCommand::Ops { .. } => {
            let ns = if args.admin { "openai_admin" } else { "openai" };
            let ops: Vec<_> = operations_for_namespace(ns)
                .map(|op| {
                    json!({
                        "operation_id": op.operation_id,
                        "method": op.method,
                        "path": op.path,
                        "cli_route": op.cli_route,
                        "request_type": op.request_type,
                        "response_type": op.response_type,
                        "transports": op.transports,
                        "deprecated": op.is_deprecated,
                        "typed_request": op.typed_request,
                        "multipart": op.is_multipart,
                        "sse": op.is_sse,
                        "binary": op.is_binary,
                    })
                })
                .collect();
            write_json(&json!({
                "provider": args.provider,
                "namespace": ns,
                "count": ops.len(),
                "operations": ops,
            }))
            .map_err(|e| e.to_string())?;
            Ok(ExitCode::Success)
        }
        OpenAiCliCommand::Call {
            operation_id,
            path_params,
            query,
            input,
        } => {
            call(
                &args.provider,
                if args.admin { "openai_admin" } else { "openai" },
                &operation_id,
                &path_params,
                &query,
                input.as_deref(),
                args.dry_run,
                args.yes,
                args.stream,
                args.output.as_deref(),
                &files,
            )
            .await
        }
        OpenAiCliCommand::Models { command } => match command {
            ModelsCommand::List { input } => {
                call(
                    &args.provider,
                    "openai",
                    "listModels",
                    &[],
                    &[],
                    input.as_deref(),
                    args.dry_run,
                    args.yes,
                    false,
                    None,
                    &files,
                )
                .await
            }
            ModelsCommand::Retrieve { model_id } => {
                call(
                    &args.provider,
                    "openai",
                    "retrieveModel",
                    &[format!("model={model_id}")],
                    &[],
                    None,
                    args.dry_run,
                    args.yes,
                    false,
                    None,
                    &files,
                )
                .await
            }
            ModelsCommand::Delete { model_id } => {
                if !args.yes && !args.dry_run {
                    return Err("delete requires --yes (or --dry-run)".into());
                }
                call(
                    &args.provider,
                    "openai",
                    "deleteModel",
                    &[format!("model={model_id}")],
                    &[],
                    None,
                    args.dry_run,
                    args.yes,
                    false,
                    None,
                    &files,
                )
                .await
            }
        },
        OpenAiCliCommand::Chat { command } => match command {
            ChatCommand::Create { input } => {
                let op = if args.stream {
                    "createChatCompletion_stream"
                } else {
                    "createChatCompletion"
                };
                // Prefer primary createChatCompletion; stream companion if present.
                let op = if args.stream && find_cli_operation("openai", op).is_none() {
                    "createChatCompletion"
                } else if args.stream {
                    op
                } else {
                    "createChatCompletion"
                };
                call(
                    &args.provider,
                    "openai",
                    op,
                    &[],
                    &[],
                    Some(&input),
                    args.dry_run,
                    args.yes,
                    args.stream,
                    args.output.as_deref(),
                    &files,
                )
                .await
            }
        },
        OpenAiCliCommand::Responses { command } => match command {
            ResponsesCommand::Create { input } => {
                call(
                    &args.provider,
                    "openai",
                    "createResponse",
                    &[],
                    &[],
                    Some(&input),
                    args.dry_run,
                    args.yes,
                    args.stream,
                    args.output.as_deref(),
                    &files,
                )
                .await
            }
        },
        OpenAiCliCommand::Embeddings { command } => match command {
            EmbeddingsCommand::Create { input } => {
                call(
                    &args.provider,
                    "openai",
                    "createEmbedding",
                    &[],
                    &[],
                    Some(&input),
                    args.dry_run,
                    args.yes,
                    false,
                    None,
                    &files,
                )
                .await
            }
        },
        OpenAiCliCommand::Admin { command } => match command {
            AdminCommand::Ops => {
                let ops: Vec<_> = operations_for_namespace("openai_admin")
                    .map(|op| op.operation_id)
                    .collect();
                write_json(&json!({"namespace": "openai_admin", "operations": ops}))
                    .map_err(|e| e.to_string())?;
                Ok(ExitCode::Success)
            }
            AdminCommand::Call {
                operation_id,
                path_params,
                query,
                input,
            } => {
                if is_mutating("openai_admin", &operation_id) && !args.yes && !args.dry_run {
                    return Err("admin mutation requires --yes confirmation (or --dry-run)".into());
                }
                call(
                    &args.provider,
                    "openai_admin",
                    &operation_id,
                    &path_params,
                    &query,
                    input.as_deref(),
                    args.dry_run,
                    args.yes,
                    false,
                    args.output.as_deref(),
                    &files,
                )
                .await
            }
        },
    }
}

fn is_mutating(namespace: &str, operation_id: &str) -> bool {
    // Fail closed: unknown metadata requires confirmation.
    super::generated_ops::operation_requires_confirmation(namespace, operation_id)
}

fn parse_files(files: &[String]) -> Result<Vec<(String, PathBuf)>, String> {
    let mut out = Vec::new();
    for f in files {
        let (field, path) = f
            .split_once('=')
            .ok_or_else(|| format!("--file must be FIELD=PATH, got `{f}`"))?;
        out.push((field.to_owned(), PathBuf::from(path)));
    }
    Ok(out)
}

fn parse_pairs(pairs: &[String]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for p in pairs {
        let (k, v) = p
            .split_once('=')
            .ok_or_else(|| format!("expected NAME=VALUE, got `{p}`"))?;
        out.push((k.to_owned(), v.to_owned()));
    }
    Ok(out)
}

async fn call(
    provider: &str,
    namespace: &str,
    operation_id: &str,
    path_params: &[String],
    query: &[String],
    input: Option<&str>,
    dry_run: bool,
    yes: bool,
    stream: bool,
    output: Option<&std::path::Path>,
    files: &[(String, PathBuf)],
) -> Result<ExitCode, String> {
    let op = find_cli_operation(namespace, operation_id).ok_or_else(|| {
        format!("unknown operation_id `{operation_id}` in namespace `{namespace}`")
    })?;
    if !op.typed_request {
        return Err(format!(
            "operation `{operation_id}` lacks a typed request binding"
        ));
    }
    // Centralized fail-closed mutation confirmation from operation metadata.
    // Unknown/missing metadata is handled by `operation_requires_confirmation`
    // (returns true). Dry-run always proceeds.
    if !dry_run && op.requires_confirmation && !yes {
        return Err(format!(
            "operation `{operation_id}` requires --yes confirmation (or --dry-run)"
        ));
    }
    if op.is_binary && output.is_none() {
        use std::io::IsTerminal;
        if std::io::stdout().is_terminal() {
            return Err(
                "binary response requires --output <path> (refusing interactive TTY)".into(),
            );
        }
    }
    let path_params = parse_pairs(path_params)?;
    let query = parse_pairs(query)?;
    let input_json = match input {
        Some(path) => {
            // Validate typed JSON exists; dispatch will deserialize to Params.
            let raw = if path == "-" {
                let mut buf = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
                    .map_err(|e| format!("read stdin: {e}"))?;
                buf
            } else {
                std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?
            };
            if raw.trim().is_empty() {
                return Err("input is empty".into());
            }
            // Structural check: must be JSON value.
            let _: Value =
                serde_json::from_str(&raw).map_err(|e| format!("input JSON invalid: {e}"))?;
            Some(raw)
        }
        None => None,
    };
    dispatch_typed_operation(
        provider,
        namespace,
        operation_id,
        &path_params,
        &query,
        input_json.as_deref(),
        dry_run,
        stream,
        output,
        files,
    )
    .await
}

/// Shared entry for OpenRouter namespace.
pub async fn call_namespace(
    provider: &str,
    namespace: &str,
    operation_id: &str,
    path_params: &[String],
    query: &[String],
    input: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Result<ExitCode, String> {
    call(
        provider,
        namespace,
        operation_id,
        path_params,
        query,
        input,
        dry_run,
        yes,
        false,
        None,
        &[],
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_models_list() {
        let args =
            OpenAiCliArgs::try_parse_from(["openai", "--provider", "openai", "models", "list"])
                .unwrap();
        assert_eq!(args.provider, "openai");
    }

    #[test]
    fn every_cli_op_is_typed() {
        for op in CLI_OPERATIONS {
            assert!(op.typed_request, "{}", op.operation_id);
            assert!(!op.request_type.is_empty());
            assert!(!op.client_method.is_empty());
            assert!(
                matches!(
                    op.method,
                    "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD"
                ),
                "bad method {} on {}",
                op.method,
                op.operation_id
            );
        }
    }

    #[test]
    fn mutation_confirmation_is_centralized_and_fail_closed() {
        use super::super::generated_ops::operation_requires_confirmation;
        // Inference-style safe exception.
        assert!(!operation_requires_confirmation(
            "openai",
            "createChatCompletion"
        ));
        // Shared-state mutation requires --yes.
        assert!(operation_requires_confirmation("openai", "deleteModel"));
        assert!(operation_requires_confirmation(
            "openai_admin",
            "admin-api-keys-create"
        ));
        // Unknown/missing metadata fails closed.
        assert!(operation_requires_confirmation(
            "openai",
            "definitelyNotARealOperation"
        ));
        // Dry-run path is tested via call() accepting dry_run without yes;
        // GET never requires confirmation.
        assert!(!operation_requires_confirmation("openai", "listModels"));
    }

    #[test]
    fn openrouter_admin_classification_on_cli_ops() {
        let key = find_cli_operation("openrouter", "getCurrentKey").expect("key");
        assert!(!key.is_admin);
        assert_eq!(key.credential_class, "application");
        let keys = CLI_OPERATIONS
            .iter()
            .find(|op| op.provider_namespace == "openrouter" && op.path.starts_with("/keys"))
            .expect("keys");
        assert!(keys.is_admin);
        assert_eq!(keys.credential_class, "admin");
    }

    #[test]
    fn dispatch_arms_bind_real_methods_for_sse_binary_multipart() {
        // Per-arm local inspection (not global substrings): each match arm body
        // must call its own client_method. Covers SSE companions, binary, and
        // multipart primaries.
        let runtime = include_str!("typed_dispatch_runtime.rs");
        let interesting: Vec<_> = CLI_OPERATIONS
            .iter()
            .filter(|op| op.is_sse || op.is_binary || op.is_multipart)
            .collect();
        assert!(
            interesting.len() >= 20,
            "expected substantial SSE/binary/multipart coverage, got {}",
            interesting.len()
        );
        for op in interesting {
            let body = extract_match_arm_body(runtime, op.operation_id)
                .unwrap_or_else(|| panic!("missing arm for {}", op.operation_id));
            assert!(
                body.contains(&format!(".{}(", op.client_method)),
                "arm {} must call .{}( locally; body snippet: {}",
                op.operation_id,
                op.client_method,
                &body[..body.len().min(200)]
            );
            // Binary arms must pass sink/output; multipart must use files.
            if op.is_binary {
                assert!(
                    body.contains("output") || body.contains("sink"),
                    "binary arm {} should pass output/sink",
                    op.operation_id
                );
            }
            if op.is_multipart {
                assert!(
                    body.contains("multipart_from") || body.contains("files"),
                    "multipart arm {} should use files",
                    op.operation_id
                );
            }
            if op.is_sse {
                assert!(
                    body.contains("events") || body.contains("_stream"),
                    "sse arm {} should stream events",
                    op.operation_id
                );
            }
        }
    }

    /// Brace-balanced body of `"operation_id" => { ... }`.
    fn extract_match_arm_body<'a>(source: &'a str, operation_id: &str) -> Option<&'a str> {
        let needle = format!("\"{operation_id}\" =>");
        let idx = source.find(&needle)?;
        let brace = source[idx..].find('{')? + idx;
        let mut depth = 0usize;
        for (i, ch) in source[brace..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&source[brace..=brace + i]);
                    }
                }
                _ => {}
            }
        }
        None
    }

    #[tokio::test]
    async fn dry_run_never_enters_live_credential_phase() {
        use super::super::typed_dispatch_runtime::{LIVE_CREDENTIAL_PHASE_COUNT, dispatch_runtime};
        use std::sync::atomic::Ordering;

        let op = find_cli_operation("openai", "listModels").expect("listModels");
        // Poison env with sentinel secrets that must never be resolved on dry-run.
        // (Resolution would not panic, but LIVE_CREDENTIAL_PHASE_COUNT proves the
        // credential phase was never entered.)
        let before = LIVE_CREDENTIAL_PHASE_COUNT.load(Ordering::SeqCst);
        let code = dispatch_runtime(
            "openai",
            op,
            &[],
            &[],
            None,
            true, // dry_run
            false,
            None,
            &[],
        )
        .await
        .expect("dry_run");
        assert_eq!(code, ExitCode::Success);
        let after = LIVE_CREDENTIAL_PHASE_COUNT.load(Ordering::SeqCst);
        assert_eq!(
            after, before,
            "dry-run must not enter live credential phase (before={before}, after={after})"
        );
    }

    #[test]
    fn typed_input_rejects_empty() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("empty.json");
        std::fs::write(&p, "").unwrap();
        let err = read_typed_input::<Value>(p.to_str().unwrap()).unwrap_err();
        assert!(err.contains("empty"));
    }
}
