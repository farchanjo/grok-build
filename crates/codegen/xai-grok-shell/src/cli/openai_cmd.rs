//! `grok openai --provider <id> ...` typed platform CLI.

use super::generated_ops::{CLI_OPERATIONS, find_cli_operation, operations_for_namespace};
use super::output::{ExitCode, read_typed_input, write_binary, write_json, write_ndjson_line};
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

    /// Dry-run shared-state mutations (print request only).
    #[arg(long)]
    pub dry_run: bool,

    /// Confirm shared-state mutations non-interactively.
    #[arg(long)]
    pub yes: bool,

    /// Write binary responses to this path.
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Emit NDJSON for streaming operations.
    #[arg(long)]
    pub stream: bool,

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
    /// operation's typed request (not raw JSON forwarding).
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
    /// Convenience: models list.
    Models {
        #[command(subcommand)]
        command: ModelsCommand,
    },
    /// Convenience: chat completions create.
    Chat {
        #[command(subcommand)]
        command: ChatCommand,
    },
    /// Convenience: responses create.
    Responses {
        #[command(subcommand)]
        command: ResponsesCommand,
    },
    /// Convenience: embeddings create.
    Embeddings {
        #[command(subcommand)]
        command: EmbeddingsCommand,
    },
    /// Administration namespace (requires --admin or admin key).
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
    /// List admin operations (never uses application keys).
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
                        "deprecated": op.is_deprecated,
                        "request_type": op.request_type,
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
            dispatch_call(
                &args.provider,
                if args.admin {
                    "openai_admin"
                } else {
                    "openai"
                },
                &operation_id,
                &path_params,
                &query,
                input.as_deref(),
                args.dry_run,
                args.yes,
                args.stream,
                args.output.as_deref(),
            )
            .await
        }
        OpenAiCliCommand::Models { command } => match command {
            ModelsCommand::List { input } => {
                dispatch_call(
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
                )
                .await
            }
            ModelsCommand::Retrieve { model_id } => {
                dispatch_call(
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
                )
                .await
            }
            ModelsCommand::Delete { model_id } => {
                if !args.yes && !args.dry_run {
                    return Err("delete requires --yes (or --dry-run)".into());
                }
                dispatch_call(
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
                )
                .await
            }
        },
        OpenAiCliCommand::Chat { command } => match command {
            ChatCommand::Create { input } => {
                dispatch_call(
                    &args.provider,
                    "openai",
                    "createChatCompletion",
                    &[],
                    &[],
                    Some(&input),
                    args.dry_run,
                    args.yes,
                    args.stream,
                    args.output.as_deref(),
                )
                .await
            }
        },
        OpenAiCliCommand::Responses { command } => match command {
            ResponsesCommand::Create { input } => {
                dispatch_call(
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
                )
                .await
            }
        },
        OpenAiCliCommand::Embeddings { command } => match command {
            EmbeddingsCommand::Create { input } => {
                dispatch_call(
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
                if is_mutating_op("openai_admin", &operation_id) && !args.yes && !args.dry_run {
                    return Err(
                        "admin mutation requires --yes confirmation (or --dry-run)".into(),
                    );
                }
                dispatch_call(
                    &args.provider,
                    "openai_admin",
                    &operation_id,
                    &path_params,
                    &query,
                    input.as_deref(),
                    args.dry_run,
                    args.yes,
                    false,
                    None,
                )
                .await
            }
        },
    }
}

fn is_mutating_op(namespace: &str, operation_id: &str) -> bool {
    find_cli_operation(namespace, operation_id)
        .map(|op| matches!(op.method, "POST" | "PUT" | "PATCH" | "DELETE"))
        .unwrap_or(true)
}

async fn dispatch_call(
    provider: &str,
    namespace: &str,
    operation_id: &str,
    path_params: &[String],
    query: &[String],
    input: Option<&str>,
    dry_run: bool,
    _yes: bool,
    stream: bool,
    output: Option<&std::path::Path>,
) -> Result<ExitCode, String> {
    let op = find_cli_operation(namespace, operation_id).ok_or_else(|| {
        format!("unknown operation_id `{operation_id}` in namespace `{namespace}`")
    })?;

    // Typed body: deserialize as JSON object (operation-specific type envelope).
    let body: Option<Value> = match input {
        Some(path) => {
            let typed: Value = read_typed_input(path)?;
            if !typed.is_object() && !typed.is_array() {
                return Err(
                    "typed request must deserialize to a JSON object or array for this operation"
                        .into(),
                );
            }
            Some(typed)
        }
        None => None,
    };

    let mut path_map = serde_json::Map::new();
    for pair in path_params {
        let (k, v) = pair
            .split_once('=')
            .ok_or_else(|| format!("path-param must be NAME=VALUE, got `{pair}`"))?;
        path_map.insert(k.to_owned(), Value::String(v.to_owned()));
    }
    let mut query_map = serde_json::Map::new();
    for pair in query {
        let (k, v) = pair
            .split_once('=')
            .ok_or_else(|| format!("query must be NAME=VALUE, got `{pair}`"))?;
        query_map.insert(k.to_owned(), Value::String(v.to_owned()));
    }

    let request_preview = json!({
        "provider": provider,
        "namespace": namespace,
        "operation_id": op.operation_id,
        "method": op.method,
        "path": op.path,
        "path_params": path_map,
        "query": query_map,
        "body": body,
        "client_method": op.client_method,
        "request_type": op.request_type,
        "dry_run": dry_run,
    });

    if dry_run {
        write_json(&request_preview).map_err(|e| e.to_string())?;
        return Ok(ExitCode::Success);
    }

    // Live execution goes through the platform client when credentials resolve.
    let result = execute_platform_call(provider, namespace, op, &path_map, &query_map, body).await?;

    if stream {
        write_ndjson_line(&result).map_err(|e| e.to_string())?;
    } else if let Some(bytes) = result.get("__binary__").and_then(|v| v.as_str()) {
        let raw = bytes.as_bytes();
        return write_binary(raw, output).map_err(|e| e.to_string());
    } else {
        write_json(&result).map_err(|e| e.to_string())?;
    }
    Ok(ExitCode::Success)
}

async fn execute_platform_call(
    provider: &str,
    namespace: &str,
    op: &super::generated_ops::CliOperation,
    path_params: &serde_json::Map<String, Value>,
    query: &serde_json::Map<String, Value>,
    body: Option<Value>,
) -> Result<Value, String> {
    use crate::provider_registry::id::ProviderId;
    use crate::provider_registry::secrets::{
        admin_key_scope, application_key_scope, read_provider_secret,
    };
    use tokio_util::sync::CancellationToken;
    use xai_grok_inference::{
        OpenAiAdminClient, OpenAiClient, PlatformClientConfig, TransportPolicy,
    };

    let home = std::env::var("GROK_HOME").unwrap_or_else(|_| {
        dirs::home_dir()
            .map(|h| h.join(".grokdev").display().to_string())
            .unwrap_or_else(|| ".".into())
    });
    let home = std::path::PathBuf::from(home);
    let (base_url, display_name) = resolve_provider_endpoint(provider, &home)?;
    let pid = ProviderId::new(provider).map_err(|e| e.to_string())?;

    let app_token = resolve_app_token(provider, &home, &pid);
    let admin_token = resolve_admin_token(provider, &home, &pid);

    let mut path = op.path.to_string();
    for (k, v) in path_params {
        let s = v.as_str().unwrap_or("");
        path = path.replace(
            &format!("{{{k}}}"),
            &xai_grok_inference::openai_platform::url_policy::encode_path_segment(s),
        );
    }
    let mut q = std::collections::BTreeMap::new();
    for (k, v) in query {
        q.insert(
            k.clone(),
            v.as_str()
                .map(|s| s.to_owned())
                .unwrap_or_else(|| v.to_string()),
        );
    }
    let method = static_method(op.method);
    let spec = xai_grok_inference::openai_platform::HttpRequestSpec {
        method,
        path,
        query: q,
        body,
        credential: if namespace == "openai_admin" {
            xai_grok_inference::openai_platform::CredentialKind::Admin
        } else {
            xai_grok_inference::openai_platform::CredentialKind::Application
        },
        expect_sse: false,
        expect_binary: false,
        multipart: false,
        operation_id: op.operation_id,
        idempotent: method == "GET" || method == "HEAD",
    };

    let cfg = PlatformClientConfig {
        provider_id: provider.to_owned(),
        display_name,
        base_url,
        admin_base_url: None,
        application_token: app_token,
        admin_token,
        extra_headers: Default::default(),
        policy: TransportPolicy::default(),
    };
    if namespace == "openai_admin" {
        let client = OpenAiAdminClient::from_config(cfg, CancellationToken::new())
            .map_err(|e| e.to_string())?;
        client
            .transport()
            .execute_json(spec)
            .await
            .map_err(|e| e.to_string())
    } else {
        let client =
            OpenAiClient::from_config(cfg, CancellationToken::new()).map_err(|e| e.to_string())?;
        client
            .transport()
            .execute_json(spec)
            .await
            .map_err(|e| e.to_string())
    }
}

fn static_method(m: &str) -> &'static str {
    match m {
        "GET" => "GET",
        "POST" => "POST",
        "PUT" => "PUT",
        "PATCH" => "PATCH",
        "DELETE" => "DELETE",
        "HEAD" => "HEAD",
        _ => "GET",
    }
}

fn resolve_app_token(
    provider: &str,
    home: &std::path::Path,
    pid: &crate::provider_registry::id::ProviderId,
) -> Option<String> {
    use crate::provider_registry::secrets::{application_key_scope, read_provider_secret};
    match provider {
        "openai" => crate::auth::read_provider_api_key(home, crate::auth::OPENAI_API_KEY_SCOPE)
            .ok()
            .flatten()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok()),
        "openrouter" => {
            crate::auth::read_provider_api_key(home, crate::auth::OPENROUTER_API_KEY_SCOPE)
                .ok()
                .flatten()
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
        }
        "zai" | "zai-model-api" => read_provider_secret(home, &application_key_scope(pid))
            .ok()
            .flatten()
            .or_else(|| std::env::var(crate::agent::zai::ZAI_ENV_KEY).ok())
            .or_else(|| std::env::var(crate::agent::zai::ZAI_TEST_ENV_KEY).ok()),
        _ => read_provider_secret(home, &application_key_scope(pid))
            .ok()
            .flatten()
            .or_else(|| {
                std::env::var(format!(
                    "GROK_PROVIDER_{}_API_KEY",
                    provider.to_ascii_uppercase()
                ))
                .ok()
            }),
    }
}

fn resolve_admin_token(
    provider: &str,
    home: &std::path::Path,
    pid: &crate::provider_registry::id::ProviderId,
) -> Option<String> {
    use crate::provider_registry::secrets::{admin_key_scope, read_provider_secret};
    read_provider_secret(home, &admin_key_scope(pid))
        .ok()
        .flatten()
        .or_else(|| {
            if provider == "openai" {
                crate::auth::read_provider_api_key(home, crate::auth::OPENAI_ADMIN_KEY_SCOPE)
                    .ok()
                    .flatten()
            } else {
                None
            }
        })
}

fn resolve_provider_endpoint(
    provider: &str,
    home: &std::path::Path,
) -> Result<(String, String), String> {
    match provider {
        "openai" => Ok((
            "https://api.openai.com/v1".into(),
            "OpenAI".into(),
        )),
        "openrouter" => Ok((
            "https://openrouter.ai/api/v1".into(),
            "OpenRouter".into(),
        )),
        "zai" | "zai-model-api" => Ok((
            crate::agent::zai::ZAI_DEFAULT_BASE_URL.into(),
            "Z.ai".into(),
        )),
        _ => {
            let cfg_path = home.join("config.toml");
            let raw = std::fs::read_to_string(&cfg_path)
                .map_err(|e| format!("read config.toml: {e}"))?;
            let val: toml::Value = raw.parse().map_err(|e| format!("parse config: {e}"))?;
            let entry = val
                .get("model_providers")
                .and_then(|t| t.get(provider))
                .ok_or_else(|| format!("provider `{provider}` not found in config.toml"))?;
            let base = entry
                .get("base_url")
                .or_else(|| entry.get("api_base_url"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("provider `{provider}` missing base_url"))?
                .to_owned();
            let name = entry
                .get("display_name")
                .and_then(|v| v.as_str())
                .unwrap_or(provider)
                .to_owned();
            Ok((base, name))
        }
    }
}

/// Shared entry point used by the OpenRouter-native namespace.
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
    dispatch_call(
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
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_models_list() {
        let args = OpenAiCliArgs::try_parse_from([
            "openai",
            "--provider",
            "openai",
            "models",
            "list",
        ])
        .unwrap();
        assert_eq!(args.provider, "openai");
        assert!(matches!(
            args.command,
            OpenAiCliCommand::Models {
                command: ModelsCommand::List { .. }
            }
        ));
    }

    #[test]
    fn every_binding_has_cli_catalog_entry() {
        assert_eq!(CLI_OPERATIONS.len(), xai_grok_inference::TOTAL_BINDING_COUNT);
        for op in CLI_OPERATIONS {
            assert!(!op.operation_id.is_empty());
            assert!(!op.cli_route.is_empty());
        }
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
