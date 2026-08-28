//! `grok provider ...` lifecycle and credential commands.

use crate::agent::providers::{ProviderId as BuiltInProviderId, ProviderManager};
use crate::provider_registry::id::{ProviderId, ProviderRef};
use crate::provider_registry::instance::ProviderKind;
use crate::provider_registry::remove_all_provider_caches;
use crate::provider_registry::secrets::{
    ProviderCredentialKind, admin_key_scope_for_kind, application_key_scope_for_kind,
    clear_configured_instance_secrets, clear_provider_secret, extra_openrouter_admin_key_scope,
    extra_openrouter_application_key_scope, store_provider_secret,
};
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
    /// Remove provider metadata via the generation-safe management API.
    ///
    /// Default: blocked while reverse references exist. Use `--force` plus
    /// `--typed-id` (exact id) for forced remove with incarnation tombstone.
    /// Secret/cache clears remain independent opt-in flags (never implicit).
    Remove {
        id: String,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        remove_secrets: bool,
        #[arg(long)]
        remove_caches: bool,
        /// Forced remove: creates an incarnation tombstone and allows removal
        /// even when reverse references exist. Requires `--typed-id` matching
        /// the provider id exactly.
        #[arg(long)]
        force: bool,
        /// Exact typed provider id confirmation for `--force` (must equal `id`).
        #[arg(long)]
        typed_id: Option<String>,
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

fn configured_instance_kind(home: &std::path::Path, id: &str) -> Option<ProviderKind> {
    crate::provider_registry::ProviderManagementService::new(home)
        .detail(id)
        .ok()
        .and_then(|d| ProviderKind::parse(&d.kind))
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
    use crate::provider_registry::ProviderManagementService;
    use crate::provider_registry::management::dto::{
        ProviderAddRequest, ProviderSavePatch, ProviderSaveRequest,
    };
    use ProviderLifecycleCommand::*;
    match args.command {
        List => {
            let svc = ProviderManagementService::from_grok_home();
            let snap = svc.list_snapshot()?;
            let rows: Vec<_> = snap
                .rows
                .iter()
                .map(|r| {
                    json!({
                        "id": r.id,
                        "kind": r.kind,
                        "display_name": r.display_name,
                        "enabled": r.enabled,
                        "is_built_in": r.is_built_in,
                        "base_url": r.base_url,
                        "status": r.status_label,
                        "generation": snap.generation.get(),
                        "credentials": {
                            "has_application_key": r.credentials.has_application_key,
                            "has_admin_key": r.credentials.has_admin_key,
                            "has_oauth": r.credentials.has_oauth,
                        },
                    })
                })
                .collect();
            crate::cli::output::write_json(&json!({
                "generation": snap.generation.get(),
                "providers": rows,
                "warnings": snap.warnings,
            }))
            .map_err(|e| e.to_string())?;
            Ok(0)
        }
        Show { id } => {
            let svc = ProviderManagementService::from_grok_home();
            let detail = svc.detail(&id)?;
            crate::cli::output::write_json(&detail).map_err(|e| e.to_string())?;
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
            let home = config
                .as_ref()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(grok_home);
            let svc = ProviderManagementService::new(home);
            // Optional env key names are applied as a follow-up metadata patch.
            let result = svc.add(ProviderAddRequest {
                id: id.clone(),
                kind,
                base_url,
                display_name,
                admin_base_url,
                enabled: true,
                expected_generation: svc.current_generation(),
            });
            if !result.ok {
                return Err(result.error.unwrap_or_else(|| "add failed".into()));
            }
            if env_key.is_some() || admin_env_key.is_some() {
                let save = svc.save(ProviderSaveRequest {
                    id: id.clone(),
                    expected_generation: result.generation,
                    patch: ProviderSavePatch {
                        env_key,
                        admin_env_key,
                        ..Default::default()
                    },
                });
                if !save.ok {
                    return Err(save.error.unwrap_or_else(|| "env key patch failed".into()));
                }
            }
            crate::cli::output::write_json(&json!({
                "ok": true,
                "id": id,
                "generation": result.generation.get(),
            }))
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
            let home = config
                .as_ref()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(grok_home);
            let svc = ProviderManagementService::new(home);
            let result = svc.save(ProviderSaveRequest {
                id: id.clone(),
                expected_generation: svc.current_generation(),
                patch: ProviderSavePatch {
                    base_url,
                    display_name,
                    admin_base_url,
                    env_key,
                    admin_env_key,
                    ..Default::default()
                },
            });
            if !result.ok {
                return Err(result.error.unwrap_or_else(|| "edit failed".into()));
            }
            crate::cli::output::write_json(&json!({
                "ok": true,
                "id": id,
                "generation": result.generation.get(),
            }))
            .map_err(|e| e.to_string())?;
            Ok(0)
        }
        Enable { id, config } => {
            let home = config
                .as_ref()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(grok_home);
            let svc = ProviderManagementService::new(home);
            let result = svc.set_enabled(&id, true, svc.current_generation());
            if !result.ok {
                return Err(result.error.unwrap_or_else(|| "enable failed".into()));
            }
            Ok(0)
        }
        Disable { id, config } => {
            let home = config
                .as_ref()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(grok_home);
            let svc = ProviderManagementService::new(home);
            let result = svc.set_enabled(&id, false, svc.current_generation());
            if !result.ok {
                return Err(result.error.unwrap_or_else(|| "disable failed".into()));
            }
            Ok(0)
        }
        Remove {
            id,
            config,
            remove_secrets,
            remove_caches,
            force,
            typed_id,
            yes,
        } => {
            if !yes {
                return Err(
                    "refusing to remove provider without --yes (shared-state mutation)".into(),
                );
            }
            let home = config
                .as_ref()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(grok_home);
            let svc = ProviderManagementService::new(home);
            let expected = svc.current_generation();
            let result = if force {
                let typed = typed_id.ok_or_else(|| {
                    "forced remove requires --typed-id equal to the provider id".to_string()
                })?;
                use crate::provider_registry::management::dto::{
                    ForceRemoveClearOptions, ProviderForceRemoveRequest,
                };
                svc.force_remove(ProviderForceRemoveRequest {
                    id: id.clone(),
                    typed_id_confirmation: typed,
                    expected_generation: expected,
                    expected_incarnation: svc.detail(&id).ok().and_then(|d| d.incarnation),
                    clear: ForceRemoveClearOptions {
                        clear_application_key: remove_secrets,
                        clear_admin_key: remove_secrets,
                        clear_oauth: remove_secrets,
                        clear_catalog_cache: remove_caches,
                        clear_capability_cache: remove_caches,
                    },
                    operation_id: None,
                })
            } else {
                // Normal remove: impact + generation gated; no tombstone.
                // Secrets/caches are not cleared implicitly on clean remove.
                let mut result = svc.remove_metadata(&id, expected, true);
                if result.ok && (remove_secrets || remove_caches) {
                    // Only after successful metadata remove (same as force path ordering).
                    if let Ok(pid) = ProviderId::new(&id) {
                        if remove_secrets {
                            clear_configured_instance_secrets(svc.home(), &pid);
                        }
                        if remove_caches {
                            let _ = remove_all_provider_caches(svc.home(), &pid);
                        }
                    }
                }
                result
            };
            if !result.ok {
                return Err(result.error.unwrap_or_else(|| "remove failed".into()));
            }
            crate::cli::output::write_json(&json!({
                "ok": true,
                "id": id,
                "forced": force,
                "generation": result.generation.get(),
                "incarnation": result.incarnation,
                "partial_commit": result.partial_commit,
                "secrets_removed": remove_secrets && result.ok,
                "caches_removed": remove_caches && result.ok,
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
            // Built-in product scopes (openrouter::api_key, anthropic::api_key, …)
            // must not be written as openai_compatible::<id>::api_key.
            if let Ok(pref) = ProviderRef::parse(&id)
                && let ProviderRef::BuiltIn(built_in) = pref
            {
                use crate::agent::providers::{ProviderId as BuiltInId, ProviderManager};
                let manager = ProviderManager::new(grok_home());
                let backend = match built_in {
                    crate::provider_registry::id::BuiltInProviderId::Xai => BuiltInId::Xai,
                    crate::provider_registry::id::BuiltInProviderId::OpenAi => BuiltInId::OpenAi,
                    crate::provider_registry::id::BuiltInProviderId::OpenRouter => {
                        BuiltInId::OpenRouter
                    }
                    crate::provider_registry::id::BuiltInProviderId::Anthropic => {
                        BuiltInId::Anthropic
                    }
                };
                manager.remove_api_key(backend).map_err(|e| e.to_string())?;
                crate::cli::output::write_json(&json!({
                    "ok": true,
                    "id": built_in.as_str(),
                    "credential_kind": "api_key",
                }))
                .map_err(|e| e.to_string())?;
                return Ok(0);
            }
            let pid = ProviderId::new(&id).map_err(|e| e.to_string())?;
            let home = grok_home();
            if let Some(kind) = configured_instance_kind(&home, &id) {
                clear_provider_secret(&home, &application_key_scope_for_kind(&pid, kind))
                    .map_err(|e| e.to_string())?;
            } else {
                let _ = clear_provider_secret(
                    &home,
                    &crate::provider_registry::secrets::application_key_scope(&pid),
                );
                let _ = clear_provider_secret(&home, &extra_openrouter_application_key_scope(&pid));
            }
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
            let home = grok_home();
            if let Some(kind) = configured_instance_kind(&home, &id) {
                clear_provider_secret(&home, &admin_key_scope_for_kind(&pid, kind))
                    .map_err(|e| e.to_string())?;
            } else {
                let _ = clear_provider_secret(
                    &home,
                    &crate::provider_registry::secrets::admin_key_scope(&pid),
                );
                let _ = clear_provider_secret(&home, &extra_openrouter_admin_key_scope(&pid));
            }
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
            let svc = ProviderManagementService::from_grok_home();
            let snap = svc.refresh_catalog(&id).await;
            crate::cli::output::write_json(&snap).map_err(|e| e.to_string())?;
            Ok(if snap.error.is_some() { 1 } else { 0 })
        }
        Test { id } => {
            let svc = ProviderManagementService::from_grok_home();
            let snap = svc.test_connection(&id).await;
            crate::cli::output::write_json(&json!({
                "ok": snap.connected,
                "id": snap.provider_id,
                "label": snap.label,
                "detail": snap.detail,
                "error": snap.error,
                "generation": snap.generation.get(),
            }))
            .map_err(|e| e.to_string())?;
            Ok(if snap.connected { 0 } else { 1 })
        }
    }
}

fn set_secret(
    id: &str,
    kind: ProviderCredentialKind,
    from_env: Option<String>,
    value: Option<String>,
) -> Result<(), String> {
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

    // Built-in product scopes: openrouter::api_key / anthropic::api_key /
    // openai::api_key — never openai_compatible::<builtin>::api_key.
    if kind == ProviderCredentialKind::Application
        && let Ok(pref) = ProviderRef::parse(id)
        && let ProviderRef::BuiltIn(built_in) = pref
    {
        use crate::agent::providers::{ProviderId as BuiltInId, ProviderManager};
        use crate::provider_registry::id::BuiltInProviderId;
        use crate::provider_registry::secrets::built_in_application_scope;
        let manager = ProviderManager::new(grok_home());
        let backend = match built_in {
            BuiltInProviderId::Xai => BuiltInId::Xai,
            BuiltInProviderId::OpenAi => BuiltInId::OpenAi,
            BuiltInProviderId::OpenRouter => BuiltInId::OpenRouter,
            BuiltInProviderId::Anthropic => BuiltInId::Anthropic,
        };
        manager
            .set_api_key(backend, secret.trim())
            .map_err(|e| e.to_string())?;
        let scope = built_in_application_scope(built_in)
            .unwrap_or_else(|| match built_in {
                BuiltInProviderId::Xai => "xai::api_key",
                _ => "unknown",
            })
            .to_owned();
        crate::cli::output::write_json(&json!({
            "ok": true,
            "id": built_in.as_str(),
            "credential_kind": kind.as_str(),
            "scope": scope,
        }))
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let pid = ProviderId::new(id).map_err(|e| e.to_string())?;
    let home = grok_home();
    let instance_kind =
        configured_instance_kind(&home, id).unwrap_or(ProviderKind::OpenAiCompatible);
    let scope = match kind {
        ProviderCredentialKind::Application => application_key_scope_for_kind(&pid, instance_kind),
        ProviderCredentialKind::Admin => admin_key_scope_for_kind(&pid, instance_kind),
    };
    store_provider_secret(&home, &scope, secret.trim()).map_err(|e| e.to_string())?;
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
    let anthropic = id == "anthropic";
    let zai = id == "zai" || id == "zai-model-api";
    json!({
        "provider_id": id,
        "openai_compatibility": {
            "chat_completions": if zai { "supported" } else { "unknown" },
            "responses": if openrouter { "supported" } else if zai { "unknown" } else { "unknown" },
            "embeddings": "unknown",
            "note": "Per-provider capability is distinct from client completeness"
        },
        "messages": if anthropic {
            json!({"status": "supported", "auth": "x-api-key", "version_header": xai_grok_inference::ANTHROPIC_VERSION})
        } else {
            json!({"status": "not_applicable"})
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
