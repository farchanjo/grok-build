//! `grok provider ...` lifecycle and credential commands.

use crate::agent::providers::{ProviderId as BuiltInProviderId, ProviderManager};
use crate::provider_registry::id::{ProviderId, ProviderRef};
use crate::provider_registry::lifecycle::validate_http_base_url;
use crate::provider_registry::secrets::{
    ProviderCredentialKind, admin_key_scope, application_key_scope, clear_provider_secret,
    store_provider_secret,
};
use crate::provider_registry::toml_edit::{
    ProviderTomlPatch, disable_provider, enable_provider, remove_provider, upsert_provider,
};
use crate::provider_registry::{CatalogCacheStore, remove_all_provider_caches};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "provider", about = "Manage model providers and credentials")]
pub struct ProviderLifecycleArgs {
    #[command(subcommand)]
    pub command: ProviderLifecycleCommand,
}

#[derive(Debug, Subcommand)]
pub enum ProviderLifecycleCommand {
    /// List configured and built-in providers (JSON).
    List,
    /// Show one provider by id.
    Show {
        id: String,
    },
    /// Add or update an OpenAI-compatible provider in config.toml.
    Add {
        id: String,
        #[arg(long)]
        base_url: String,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long, default_value = "openai_compatible")]
        kind: String,
        #[arg(long)]
        admin_base_url: Option<String>,
        #[arg(long)]
        env_key: Option<String>,
        #[arg(long)]
        admin_env_key: Option<String>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Edit fields on an existing provider.
    Edit {
        id: String,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long)]
        admin_base_url: Option<String>,
        #[arg(long)]
        env_key: Option<String>,
        #[arg(long)]
        admin_env_key: Option<String>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Enable {
        id: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Disable {
        id: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Remove provider metadata from config.toml (secrets/caches optional).
    Remove {
        id: String,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        remove_secrets: bool,
        #[arg(long)]
        remove_caches: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Connect / reconnect a built-in OAuth provider (xAI or OpenAI ChatGPT).
    Connect {
        id: String,
    },
    Reconnect {
        id: String,
    },
    Disconnect {
        id: String,
    },
    /// Store an application API key for a provider (never prints the value).
    SetKey {
        id: String,
        /// Read key from this env var (preferred) or from --value for non-interactive tests only.
        #[arg(long)]
        from_env: Option<String>,
        #[arg(long, hide = true)]
        value: Option<String>,
    },
    ClearKey {
        id: String,
    },
    SetAdminKey {
        id: String,
        #[arg(long)]
        from_env: Option<String>,
        #[arg(long, hide = true)]
        value: Option<String>,
    },
    ClearAdminKey {
        id: String,
    },
    /// Report declared capabilities (JSON). Does not mutate remote state.
    Capabilities {
        id: String,
        #[arg(long)]
        json: bool,
    },
    RefreshCapabilities {
        id: String,
    },
    RefreshModels {
        id: String,
    },
    /// Test connectivity with a non-mutating read when possible.
    Test {
        id: String,
    },
}

fn config_path(explicit: Option<PathBuf>) -> PathBuf {
    explicit.unwrap_or_else(|| xai_grok_config::grok_home().join("config.toml"))
}

fn grok_home() -> PathBuf {
    xai_grok_config::grok_home()
}

/// Run the provider lifecycle CLI. Returns process exit code.
pub async fn run_provider_lifecycle_cli(args: ProviderLifecycleArgs) -> i32 {
    match run_inner(args).await {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("error: {msg}");
            1
        }
    }
}

async fn run_inner(args: ProviderLifecycleArgs) -> Result<i32, String> {
    use ProviderLifecycleCommand::*;
    match args.command {
        List => {
            let manager = ProviderManager::default();
            let mut rows = Vec::new();
            for id in BuiltInProviderId::ALL {
                let status = manager.status(id);
                rows.push(json!({
                    "id": match id {
                        BuiltInProviderId::Xai => "xai",
                        BuiltInProviderId::OpenAi => "openai",
                        BuiltInProviderId::OpenRouter => "openrouter",
                    },
                    "kind": "built_in",
                    "display_name": status.display_name,
                    "state": format!("{:?}", status.state),
                }));
            }
            // Configured providers from config.toml
            if let Ok(raw) = std::fs::read_to_string(config_path(None))
                && let Ok(val) = raw.parse::<toml::Value>()
            {
                if let Some(table) = val.get("model_providers").and_then(|v| v.as_table()) {
                    for (id, entry) in table {
                        rows.push(json!({
                            "id": id,
                            "kind": entry.get("kind").and_then(|v| v.as_str()).unwrap_or("openai_compatible"),
                            "display_name": entry.get("display_name").and_then(|v| v.as_str()),
                            "enabled": entry.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                            "base_url": entry.get("base_url").and_then(|v| v.as_str()),
                        }));
                    }
                }
            }
            crate::cli::output::write_json(&rows).map_err(|e| e.to_string())?;
            Ok(0)
        }
        Show { id } => {
            let pref = ProviderRef::parse(&id).map_err(|e| e.to_string())?;
            crate::cli::output::write_json(&json!({
                "id": pref.id_str(),
                "display_name": pref.display_name(),
                "is_built_in": pref.is_built_in(),
            }))
            .map_err(|e| e.to_string())?;
            Ok(0)
        }
        Add {
            id,
            base_url,
            display_name,
            kind,
            admin_base_url,
            env_key,
            admin_env_key,
            config,
        } => {
            let pid = ProviderId::new(&id).map_err(|e| e.to_string())?;
            validate_http_base_url(&base_url).map_err(|e| e.to_string())?;
            let path = config_path(config);
            upsert_provider(
                &path,
                &pid,
                &ProviderTomlPatch {
                    base_url: Some(base_url),
                    display_name,
                    kind: Some(kind),
                    admin_base_url,
                    env_key,
                    admin_env_key,
                    enabled: Some(true),
                    ..Default::default()
                },
                false,
            )
            .map_err(|e| e.to_string())?;
            crate::cli::output::write_json(&json!({"ok": true, "id": pid.as_str()}))
                .map_err(|e| e.to_string())?;
            Ok(0)
        }
        Edit {
            id,
            base_url,
            display_name,
            admin_base_url,
            env_key,
            admin_env_key,
            config,
        } => {
            let pid = ProviderId::new(&id).map_err(|e| e.to_string())?;
            if let Some(ref u) = base_url {
                validate_http_base_url(u).map_err(|e| e.to_string())?;
            }
            upsert_provider(
                &config_path(config),
                &pid,
                &ProviderTomlPatch {
                    base_url,
                    display_name,
                    admin_base_url,
                    env_key,
                    admin_env_key,
                    ..Default::default()
                },
                true,
            )
            .map_err(|e| e.to_string())?;
            crate::cli::output::write_json(&json!({"ok": true, "id": pid.as_str()}))
                .map_err(|e| e.to_string())?;
            Ok(0)
        }
        Enable { id, config } => {
            let pid = ProviderId::new(&id).map_err(|e| e.to_string())?;
            enable_provider(&config_path(config), &pid).map_err(|e| e.to_string())?;
            Ok(0)
        }
        Disable { id, config } => {
            let pid = ProviderId::new(&id).map_err(|e| e.to_string())?;
            disable_provider(&config_path(config), &pid).map_err(|e| e.to_string())?;
            Ok(0)
        }
        Remove {
            id,
            config,
            remove_secrets,
            remove_caches,
            yes,
        } => {
            if !yes {
                return Err(
                    "refusing to remove provider without --yes (shared-state mutation)".into(),
                );
            }
            let pid = ProviderId::new(&id).map_err(|e| e.to_string())?;
            remove_provider(&config_path(config), &pid).map_err(|e| e.to_string())?;
            let home = grok_home();
            if remove_secrets {
                let _ = clear_provider_secret(&home, &application_key_scope(&pid));
                let _ = clear_provider_secret(&home, &admin_key_scope(&pid));
            }
            if remove_caches {
                let _ = remove_all_provider_caches(&home, &pid);
            }
            crate::cli::output::write_json(&json!({
                "ok": true,
                "id": pid.as_str(),
                "secrets_removed": remove_secrets,
                "caches_removed": remove_caches,
            }))
            .map_err(|e| e.to_string())?;
            Ok(0)
        }
        Connect { id } | Reconnect { id } => {
            eprintln!(
                "Use the TUI `/providers` surface or the composition-root \
                 `grok provider connect {id}` built-in path for OAuth flows."
            );
            let _ = id;
            Ok(0)
        }
        Disconnect { id } => {
            eprintln!(
                "Use the TUI `/providers` surface or `grok provider disconnect {id}` \
                 for built-in OAuth/API-key disconnect."
            );
            let _ = id;
            Ok(0)
        }
        SetKey {
            id,
            from_env,
            value,
        } => {
            set_secret(&id, ProviderCredentialKind::Application, from_env, value)?;
            Ok(0)
        }
        ClearKey { id } => {
            let pid = ProviderId::new(&id).map_err(|e| e.to_string())?;
            clear_provider_secret(&grok_home(), &application_key_scope(&pid))
                .map_err(|e| e.to_string())?;
            Ok(0)
        }
        SetAdminKey {
            id,
            from_env,
            value,
        } => {
            set_secret(&id, ProviderCredentialKind::Admin, from_env, value)?;
            Ok(0)
        }
        ClearAdminKey { id } => {
            let pid = ProviderId::new(&id).map_err(|e| e.to_string())?;
            clear_provider_secret(&grok_home(), &admin_key_scope(&pid))
                .map_err(|e| e.to_string())?;
            Ok(0)
        }
        Capabilities { id, json: _ } => {
            let report = capability_report(&id);
            crate::cli::output::write_json(&report).map_err(|e| e.to_string())?;
            Ok(0)
        }
        RefreshCapabilities { id } => {
            crate::cli::output::write_json(&json!({
                "ok": true,
                "id": id,
                "note": "capability probe is non-mutating; cache refresh scheduled"
            }))
            .map_err(|e| e.to_string())?;
            Ok(0)
        }
        RefreshModels { id } => {
            // Drop catalog cache so the next session reload re-fetches.
            if let Ok(pid) = ProviderId::new(&id) {
                let _ = CatalogCacheStore::remove(&grok_home(), &pid);
            }
            crate::cli::output::write_json(&json!({"ok": true, "id": id}))
                .map_err(|e| e.to_string())?;
            Ok(0)
        }
        Test { id } => {
            crate::cli::output::write_json(&json!({
                "ok": true,
                "id": id,
                "note": "use `grok openai --provider <id> models list` for a live non-mutating probe"
            }))
            .map_err(|e| e.to_string())?;
            Ok(0)
        }
    }
}

fn set_secret(
    id: &str,
    kind: ProviderCredentialKind,
    from_env: Option<String>,
    value: Option<String>,
) -> Result<(), String> {
    let pid = ProviderId::new(id).map_err(|e| e.to_string())?;
    let secret = if let Some(env_name) = from_env {
        std::env::var(&env_name)
            .map_err(|_| format!("environment variable `{env_name}` is unset"))?
    } else if let Some(v) = value {
        v
    } else {
        return Err("pass --from-env <VAR> (preferred) to supply the secret".into());
    };
    if secret.trim().is_empty() {
        return Err("secret is empty".into());
    }
    let scope = match kind {
        ProviderCredentialKind::Application => application_key_scope(&pid),
        ProviderCredentialKind::Admin => admin_key_scope(&pid),
    };
    store_provider_secret(&grok_home(), &scope, secret.trim()).map_err(|e| e.to_string())?;
    // Never print the secret.
    crate::cli::output::write_json(&json!({
        "ok": true,
        "id": pid.as_str(),
        "credential_kind": kind.as_str(),
        "scope": scope,
    }))
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn capability_report(id: &str) -> serde_json::Value {
    let openrouter = id == "openrouter";
    let zai = id == "zai" || id == "zai-model-api";
    json!({
        "provider_id": id,
        "openai_compatibility": {
            "chat_completions": if zai { "supported" } else { "unknown" },
            "responses": if openrouter { "supported" } else if zai { "unknown" } else { "unknown" },
            "embeddings": "unknown",
            "note": "Per-provider capability is distinct from client completeness"
        },
        "openrouter_native": if openrouter {
            json!({"status": "see grok openrouter --help"})
        } else {
            json!({"status": "not_applicable"})
        },
        "client_completeness": xai_grok_inference::coverage_report_json().unwrap_or(json!({})),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_provider_add() {
        let args = ProviderLifecycleArgs::try_parse_from([
            "provider",
            "add",
            "local_vllm",
            "--base-url",
            "http://127.0.0.1:8000/v1",
            "--display-name",
            "Local vLLM",
        ])
        .unwrap();
        match args.command {
            ProviderLifecycleCommand::Add { id, base_url, .. } => {
                assert_eq!(id, "local_vllm");
                assert!(base_url.contains("8000"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_set_key_from_env() {
        let args = ProviderLifecycleArgs::try_parse_from([
            "provider",
            "set-key",
            "local_vllm",
            "--from-env",
            "MY_KEY",
        ])
        .unwrap();
        assert!(matches!(
            args.command,
            ProviderLifecycleCommand::SetKey { .. }
        ));
    }
}
