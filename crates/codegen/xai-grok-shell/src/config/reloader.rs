use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use crate::auth::{GrokAuth, read_auth_json};

use super::watcher::ConfigChangeEvent;

/// Typed, `Send`-safe messages for the agent to apply inside its `LocalSet`.
#[derive(Debug)]
pub enum ConfigUpdate {
    /// New auth credentials from disk.
    Auth(Box<GrokAuth>),
    /// Auth scope was removed (user logged out).
    AuthCleared,
    /// A **broadcast** MCP reload — applies to every active session
    /// regardless of cwd. Fires for two cases:
    ///
    /// 1. The global `[mcp_servers]` table in `~/.grok/config.toml`
    ///    changed.
    /// 2. The user's home-level `~/.claude.json` changed.
    ///    `load_claude_json_mcp_servers_as_configs` reads this file
    ///    for every session, so the reload cannot be narrowed by cwd.
    ///
    /// Project-scoped changes (`<cwd>/.grok/config.toml`,
    /// `<cwd>/.mcp.json`, project-level `<cwd>/.claude.json`) emit
    /// [`Self::ProjectMcpServersChanged`] instead so the reload can
    /// be narrowed to matching cwds.
    ///
    /// Deliberately kept as a unit variant.
    /// Adding a payload here would force pattern-match updates across
    /// (`<cwd>/.grok/config.toml`, `<cwd>/.mcp.json`, or
    /// `mvp_agent`, `app`, `session/handle`, etc.
    McpServersChanged,
    /// A **project-scoped** MCP config file changed
    /// `<cwd>/.claude.json`). Agent should reload MCP only for
    /// sessions whose cwd matches `cwd` (or sits beneath it).
    ///
    /// Strictly additive to [`Self::McpServersChanged`] — the unit
    /// variant continues to fire for global-config edits. The two
    /// cases are split so per-project reloads don't
    /// grok process sharing the home dir). The agent should consult the cache
    /// thrash unrelated sessions.
    ProjectMcpServersChanged {
        /// The project root whose `.grok/`, `.mcp.json`, or
        /// `.claude.json` file was edited. Sessions whose cwd equals
        /// this path — or is a descendant of it — are the reload
        /// targets.
        cwd: PathBuf,
    },
    /// Updated memory config (boxed to avoid large enum variant).
    Memory(Box<crate::config::MemoryConfig>),
    /// Updated skills discovery config.
    Skills(xai_grok_agent::prompt::skills::SkillsConfig),
    /// Updated `[compat]` vendor-compatibility config. Applied on the
    /// next agent (re)build, which re-resolves `compat_resolved`.
    Compat(Box<xai_grok_tools::types::compat::CompatConfigToml>),
    /// The `[model.*]` entries in config.toml changed. Agent should re-resolve
    /// its model list (BYOK models added/removed, default or surprise changed).
    ModelsChanged,
    /// `~/.grok/models_cache.json` was rewritten on disk (possibly by another
    /// via `ModelsManager::reload_from_disk_cache`, which content-dedupes
    /// self-writes (`persist` / `renew_ttl`) before applying. No payload —
    /// validation (TTL, version, auth method) requires `ModelsManager` state
    /// drop redundant `ProjectMcpServersChanged` dispatches on
    /// the reloader doesn't have.
    ModelsCacheChanged,
    /// Updated UI settings — agent broadcasts `x.ai/config_changed` to IPC clients.
    Ui {
        theme: Option<String>,
        yolo: bool,
        fork_secondary_model: Option<String>,
    },
    /// Updated compaction configuration — strategy, trigger policy,
    /// band count, and model routes. Broadcasts to all sessions for
    /// live policy mutation.
    Compaction(Box<crate::agent::config::CompactionConfig>),
    /// Named retrieval graph sections changed (`embedding_models`,
    /// `reranker_models`, `retrieval_profiles`, `prime`, or
    /// `[memory] retrieval_profile`). Agent rebuilds the PR17 registry
    /// snapshot (last-known-good retained on invalid candidates).
    RetrievalGraphChanged,
    /// One coalesced reload intent for a watcher batch that touched model/
    /// provider config, the retrieval graph, and/or `models_cache.json`.
    /// Flags record which inputs need a reload; the agent/session-admin lane
    /// owns candidate build, validation, publication, and notification.
    ModelReload {
        /// `[model.*]`, `[models]`, `[model_providers]`, or `[media]` changed.
        models: bool,
        /// `[model_providers]` changed and must join catalog/retrieval publish.
        providers: bool,
        /// Named retrieval graph sections changed.
        retrieval: bool,
        /// `~/.grok/models_cache.json` was rewritten on disk.
        cache: bool,
    },
}

/// Sections the reloader diffs in a global-config change that route through
/// the agent/session-admin reload lane (models/retrieval only). Other sections
/// (MCP, memory, skills, UI, compaction) are dispatched directly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ConfigReloadDiffs {
    models: bool,
    providers: bool,
    retrieval: bool,
}

fn coalesced_reload_intent(diffs: ConfigReloadDiffs, cache: bool) -> Option<ConfigUpdate> {
    (diffs.models || diffs.retrieval || cache).then_some(ConfigUpdate::ModelReload {
        models: diffs.models,
        providers: diffs.providers,
        retrieval: diffs.retrieval,
        cache,
    })
}

/// Runs on `tokio::spawn` (`Send`). Receives raw [`ConfigChangeEvent`]s from
/// the file watcher, diffs against last-known state, and sends [`ConfigUpdate`]
/// messages to the agent via an `mpsc` channel.
pub struct ConfigReloader {
    last_auth_key_hash: u64,
    last_global_config: toml::Value,
    /// Per-cwd content hash of the project MCP config files, used to
    /// to diff (the dedup lives in `ModelsManager::reload_from_disk_cache`),
    /// mtime-only touches (see `hash_project_mcp_config`).
    last_project_mcp_hashes: HashMap<PathBuf, u64>,
    /// Content hash of the `[compaction]` section for dedup.
    last_compaction_hash: u64,
    grok_home: PathBuf,
    auth_scope: String,
    remote_settings: Option<crate::util::config::RemoteSettings>,
    config_update_tx: mpsc::UnboundedSender<ConfigUpdate>,
    /// Whether --experimental-memory was passed at startup. Persists across config reloads.
    experimental_memory: bool,
    /// Whether --no-memory was passed at startup. Persists across config reloads.
    no_memory: bool,
}

impl ConfigReloader {
    pub fn new(
        grok_home: PathBuf,
        initial_auth_key_hash: u64,
        initial_config: toml::Value,
        auth_scope: String,
        remote_settings: Option<crate::util::config::RemoteSettings>,
        config_update_tx: mpsc::UnboundedSender<ConfigUpdate>,
        experimental_memory: bool,
        no_memory: bool,
    ) -> Self {
        // Compute initial compaction hash from the initial config
        let initial_compaction_hash = hash_compaction_config(&initial_config);
        Self {
            last_auth_key_hash: initial_auth_key_hash,
            last_global_config: initial_config,
            last_project_mcp_hashes: HashMap::new(),
            last_compaction_hash: initial_compaction_hash,
            grok_home,
            auth_scope,
            remote_settings,
            config_update_tx,
            experimental_memory,
            no_memory,
        }
    }

    /// Main loop. Batches all events from each debounce tick before processing.
    pub async fn run(
        mut self,
        mut events: mpsc::UnboundedReceiver<ConfigChangeEvent>,
        cancel: CancellationToken,
    ) {
        loop {
            let first = tokio::select! {
                biased;
                _ = cancel.cancelled() => break,
                evt = events.recv() => match evt {
                    Some(e) => e,
                    None => break,
                },
            };

            // Drain additional events that arrived in the same tick
            let mut batch = vec![first];
            while let Ok(evt) = events.try_recv() {
                batch.push(evt);
            }

            let has_auth = batch
                .iter()
                .any(|e| matches!(e, ConfigChangeEvent::AuthChanged));
            let has_global_config = batch
                .iter()
                .any(|e| matches!(e, ConfigChangeEvent::GlobalConfigChanged));
            let has_project_config = batch
                .iter()
                .any(|e| matches!(e, ConfigChangeEvent::ProjectConfigChanged { .. }));
            // `~/.claude.json` is loaded by every
            // session (it does NOT live in a project root), so its
            // reload must broadcast through the legacy unit
            // `McpServersChanged` arm. Routing it through the per-
            // cwd variant would silently miss sessions outside `$HOME`.
            let has_home_claude_json = batch
                .iter()
                .any(|e| matches!(e, ConfigChangeEvent::HomeClaudeJsonChanged));
            let has_models_cache = batch
                .iter()
                .any(|e| matches!(e, ConfigChangeEvent::ModelsCacheChanged));
            let has_config = has_global_config || has_project_config;

            // Collect the unique cwds whose project
            // files changed so we can emit one
            // `ConfigUpdate::ProjectMcpServersChanged { cwd }` per
            // project root (rather than the legacy unit
            // `McpServersChanged` that swept every session).
            let project_cwds = collect_project_cwds(&batch);

            if has_auth {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.reload_auth()));
                match result {
                    Ok(Err(e)) => {
                        error!(error = %e, "auth hot-reload failed, keeping previous credentials");
                        // Whole-file deletion (NotFound) and corrupt JSON
                        // land here. The resulting memory/disk divergence
                        // must be visible in unified.jsonl.
                        let path = self.grok_home.join("auth.json");
                        xai_grok_telemetry::unified_log::error(
                            "auth reload: auth.json unreadable, keeping previous credentials",
                            None,
                            Some(serde_json::json!({
                                "error": e.to_string(),
                                "path": path.display().to_string(),
                                "path_exists": path.exists(),
                            })),
                        );
                    }
                    Err(_) => {
                        error!("panic in auth reload handler, keeping previous credentials");
                    }
                    Ok(Ok(())) => {}
                }
            }

            // Coalesce all catalog and graph inputs from this watcher batch.
            let mut diffs = ConfigReloadDiffs::default();
            if has_config {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.reload_config()));
                match result {
                    Ok(Err(e)) => {
                        error!(error = %e, "config hot-reload failed, keeping last-known-good");
                    }
                    Err(_) => {
                        error!("panic in config reload handler, keeping last-known-good");
                    }
                    Ok(Ok(loaded)) => diffs = loaded,
                }
            }

            // NB: the legacy fall-through that emitted a unit
            // `McpServersChanged` for any project `.mcp.json` /
            // `.claude.json` change is replaced by the
            // per-cwd fan-out below — `collect_project_cwds` already
            // includes every `McpConfigChanged` path in `project_cwds`,
            // so a separate emit here would double-dispatch. Global
            // `[mcp_servers]` edits are dispatched inside `reload_config`.

            // Home-level `~/.claude.json` must
            // broadcast to every session through the unit variant —
            // sessions outside `$HOME` would otherwise be silently
            // skipped by the per-cwd `cwd_matches` filter.
            if has_home_claude_json {
                info!("~/.claude.json change detected — broadcasting MCP reload");
                let _ = self.config_update_tx.send(ConfigUpdate::McpServersChanged);
            }

            // Fan out one
            // `ProjectMcpServersChanged { cwd }` per affected project
            // root. The legacy unit `McpServersChanged` above stays
            // for global-config edits — both variants can fire in the
            // same tick (e.g. `~/.grok/config.toml` AND
            // `<cwd>/.mcp.json` edited together).
            for cwd in project_cwds {
                // Skip the dispatch when the project config bytes are
                // unchanged (the watcher fires on mtime-only touches).
                // On any uncertainty we dispatch; see
                // `hash_project_mcp_config`.
                let new_hash = hash_project_mcp_config(&cwd);
                let unchanged = match (new_hash, self.last_project_mcp_hashes.get(&cwd)) {
                    (Some(new), Some(&prev)) => new == prev,
                    _ => false,
                };
                if unchanged {
                    debug!(
                        cwd = %cwd.display(),
                        "project MCP config event with unchanged content, skipping reload"
                    );
                    continue;
                }
                if let Some(h) = new_hash {
                    self.last_project_mcp_hashes.insert(cwd.clone(), h);
                }
                info!("project MCP config change detected");
                let _ = self
                    .config_update_tx
                    .send(ConfigUpdate::ProjectMcpServersChanged { cwd });
            }

            if let Some(update) = coalesced_reload_intent(diffs, has_models_cache) {
                info!(
                    models = diffs.models,
                    providers = diffs.providers,
                    retrieval = diffs.retrieval,
                    cache = has_models_cache,
                    "coalesced model/retrieval/cache reload intent"
                );
                let _ = self.config_update_tx.send(update);
            }
        }
    }

    fn reload_auth(&mut self) -> anyhow::Result<()> {
        let auth_path = self.grok_home.join("auth.json");
        let store = read_auth_json(&auth_path)?;

        match crate::auth::lookup_auth(&store, &self.auth_scope) {
            Some(auth) => {
                let new_hash = hash_auth_key(&auth.key);

                if new_hash == self.last_auth_key_hash {
                    debug!("auth.json changed but token key is identical, skipping");
                    return Ok(());
                }

                self.last_auth_key_hash = new_hash;
                let _ = self
                    .config_update_tx
                    .send(ConfigUpdate::Auth(Box::new(auth.clone())));
                info!("auth token change detected, sent update to agent");
            }
            None => {
                if self.last_auth_key_hash != 0 {
                    self.last_auth_key_hash = 0;
                    let _ = self.config_update_tx.send(ConfigUpdate::AuthCleared);
                    info!("auth scope removed from auth.json, sent clear to agent");
                    // AuthCleared makes the agent drop in-memory credentials;
                    // record what the reloader saw so "entry removed" is
                    // distinguishable from "file deleted" (the Err path).
                    xai_grok_telemetry::unified_log::warn(
                        "auth reload: scope entry gone, sending AuthCleared",
                        None,
                        Some(serde_json::json!({
                            "scope": &self.auth_scope,
                            "scopes_on_disk": store.keys().collect::<Vec<_>>(),
                        })),
                    );
                }
            }
        }
        Ok(())
    }

    fn reload_config(&mut self) -> anyhow::Result<ConfigReloadDiffs> {
        let new_global = match crate::config::load_from_disk() {
            Ok(value) => value,
            Err(error) => {
                error!(%error, "failed to parse config.toml, keeping last-known-good");
                return Ok(ConfigReloadDiffs::default());
            }
        };
        Ok(self.reload_config_value(new_global))
    }

    fn reload_config_value(&mut self, new_global: toml::Value) -> ConfigReloadDiffs {
        let mut diffs = ConfigReloadDiffs::default();

        // MCP servers — compare [mcp_servers] table in the **global**
        // config (`~/.grok/config.toml`) via toml::Value. Project-
        // scoped changes (`<cwd>/.grok/config.toml`,
        // `<cwd>/.mcp.json`) are dispatched separately via
        // `ConfigUpdate::ProjectMcpServersChanged { cwd }` (see
        // `collect_project_cwds`) so they don't sweep
        // unrelated sessions.
        let old_mcp_table = self.last_global_config.get("mcp_servers");
        let new_mcp_table = new_global.get("mcp_servers");
        let mcp_changed = old_mcp_table != new_mcp_table;
        if mcp_changed {
            info!("Global MCP server config change detected");
            let _ = self.config_update_tx.send(ConfigUpdate::McpServersChanged);
        }

        // Memory config
        let old_mem = crate::config::MemoryConfig::resolve(
            self.experimental_memory,
            self.no_memory,
            &self.last_global_config,
            self.remote_settings.as_ref(),
        );
        let new_mem = crate::config::MemoryConfig::resolve(
            self.experimental_memory,
            self.no_memory,
            &new_global,
            self.remote_settings.as_ref(),
        );
        if old_mem != new_mem {
            info!("memory config change detected");
            let _ = self
                .config_update_tx
                .send(ConfigUpdate::Memory(Box::new(new_mem)));
        }

        // Skills config
        let old_skills = parse_skills_config(&self.last_global_config);
        let new_skills = parse_skills_config(&new_global);
        if old_skills != new_skills {
            info!("skills config change detected");
            let _ = self.config_update_tx.send(ConfigUpdate::Skills(new_skills));
        }

        // Compat config ([compat] vendor toggles)
        let old_compat = parse_compat_config(&self.last_global_config);
        let new_compat = parse_compat_config(&new_global);
        if old_compat != new_compat {
            info!("compat config change detected");
            let _ = self
                .config_update_tx
                .send(ConfigUpdate::Compat(Box::new(new_compat)));
        }

        // Catalog/model policy changes do not inherently change retrieval.
        // Provider changes do: exact retrieval pins and capability validation
        // must converge atomically with the provider-backed catalog.
        let old_model_table = self.last_global_config.get("model");
        let new_model_table = new_global.get("model");
        let old_models_table = self.last_global_config.get("models");
        let new_models_table = new_global.get("models");
        let old_providers_table = self.last_global_config.get("model_providers");
        let new_providers_table = new_global.get("model_providers");
        let old_media_table = self.last_global_config.get("media");
        let new_media_table = new_global.get("media");
        let model_policy_changed = old_model_table != new_model_table
            || old_models_table != new_models_table
            || old_media_table != new_media_table;
        let providers_changed = old_providers_table != new_providers_table;
        if model_policy_changed || providers_changed {
            info!("model or model_providers config change detected");
            diffs.models = true;
        }
        if providers_changed {
            diffs.providers = true;
            diffs.retrieval = true;
        }

        // UI fields (theme, yolo, fork_secondary_model)
        let old_ui = extract_ui_fields(&self.last_global_config);
        let new_ui = extract_ui_fields(&new_global);
        if old_ui != new_ui {
            info!("UI config change detected");
            let _ = self.config_update_tx.send(ConfigUpdate::Ui {
                theme: new_ui.0,
                yolo: new_ui.1,
                fork_secondary_model: new_ui.2,
            });
        }

        self.maybe_reload_compaction(&new_global);

        // Named retrieval graph (PR15/PR17). Provider changes already fire
        // ModelsChanged; this covers retrieval-only section edits.
        if retrieval_graph_sections_changed(&self.last_global_config, &new_global) {
            info!("retrieval graph config change detected");
            diffs.retrieval = true;
        }

        self.last_global_config = new_global;
        diffs
    }

    /// Apply a changed `[compaction]` section only after both deserialization
    /// and semantic validation succeed. The hash tracks the last accepted
    /// value, not merely the last observed bytes, preserving last-known-good.
    fn maybe_reload_compaction(&mut self, config: &toml::Value) {
        let new_hash = hash_compaction_config(config);
        if self.last_compaction_hash == new_hash {
            return;
        }
        match parse_compaction_config(config) {
            Ok(compaction) => {
                info!("compaction config change detected");
                let _ = self
                    .config_update_tx
                    .send(ConfigUpdate::Compaction(Box::new(compaction)));
                self.last_compaction_hash = new_hash;
            }
            Err(error) => {
                error!(
                    %error,
                    "invalid compaction config, keeping last-known-good"
                );
                // Do not update the accepted hash. A corrected write with the
                // same prior good value must still be reconsidered.
            }
        }
    }
}

/// Derive the unique project cwds whose files were touched in this
/// debounce window. Used to fan out one
/// [`ConfigUpdate::ProjectMcpServersChanged`] per project root rather
/// than one legacy `McpServersChanged` that reloads every active
/// session.
///
/// Path-to-cwd mapping:
///
/// | `ConfigChangeEvent`        | path shape              | cwd               |
/// |----------------------------|-------------------------|-------------------|
/// | `ProjectConfigChanged`     | `<cwd>/.grok/config.toml` | `<cwd>`           |
/// | `McpConfigChanged`         | `<cwd>/.mcp.json`         | `<cwd>`           |
/// | `McpConfigChanged`         | `<cwd>/.claude.json`      | `<cwd>`           |
///
/// Order-preserving de-dup (a `Vec` rather than a `HashSet`) so the
/// downstream emit order is deterministic in tests.
/// Whether any PR15 named retrieval section differs between two config roots.
fn retrieval_graph_sections_changed(old: &toml::Value, new: &toml::Value) -> bool {
    const KEYS: &[&str] = &[
        "embedding_models",
        "reranker_models",
        "retrieval_profiles",
        "prime",
    ];
    for key in KEYS {
        if old.get(*key) != new.get(*key) {
            return true;
        }
    }
    // Optional additive memory profile selection only.
    let old_mem = old.get("memory").and_then(|m| m.get("retrieval_profile"));
    let new_mem = new.get("memory").and_then(|m| m.get("retrieval_profile"));
    old_mem != new_mem
}

fn collect_project_cwds(batch: &[ConfigChangeEvent]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for evt in batch {
        let cwd = match evt {
            ConfigChangeEvent::ProjectConfigChanged { path } => {
                // <cwd>/.grok/config.toml → <cwd>
                path.parent()
                    .and_then(|p| p.parent())
                    .map(|p| p.to_path_buf())
            }
            ConfigChangeEvent::McpConfigChanged { path } => {
                // <cwd>/.mcp.json or <cwd>/.claude.json → <cwd>
                path.parent().map(|p| p.to_path_buf())
            }
            _ => None,
        };
        if let Some(cwd) = cwd
            && !out.contains(&cwd)
        {
            out.push(cwd);
        }
    }
    out
}

/// Content hash of the cwd-dependent MCP config files a
/// `ProjectMcpServersChanged { cwd }` reload re-reads. Walks ancestors
/// up to the git root exactly as the loaders do (`find_project_configs`
/// for `.grok/config.toml`, `find_mcp_json_files` for `.mcp.json`) so
/// the hash can't drift from the set the merge actually reads, plus
/// `<cwd>/.claude.json` (watched at the project root). A stable hash
/// means the reload would be a no-op. Home-level sources
/// (`~/.grok/config.toml`, `~/.claude.json`, `~/.cursor/mcp.json`)
/// change through their own events.
///
/// Returns `None` on a non-`NotFound` read error so the caller
/// dispatches rather than risk suppressing a real edit.
fn hash_project_mcp_config(cwd: &Path) -> Option<u64> {
    let mut paths = crate::config::find_project_configs(cwd);
    paths.extend(crate::util::config::find_mcp_json_files(cwd));
    paths.push(cwd.join(".claude.json"));

    let mut hasher = DefaultHasher::new();
    paths.len().hash(&mut hasher);
    for f in &paths {
        f.to_string_lossy().hash(&mut hasher);
        match std::fs::read(f) {
            Ok(bytes) => {
                1u8.hash(&mut hasher); // present
                bytes.hash(&mut hasher);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                0u8.hash(&mut hasher); // absent
            }
            Err(_) => return None, // can't read confidently → dispatch
        }
    }
    Some(hasher.finish())
}

pub(crate) fn hash_auth_key(key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

/// Extract the `[skills]` table from an effective config.
///
/// Consumers: the reload dispatch above (change detection →
/// `ConfigUpdate::Skills`) and `grok inspect` (via the `crate::config`
/// re-export), so both honor the same paths/ignore/disabled as a live
/// session. Session spawn parses the same table separately through the typed
/// `Config.skills` (agent/config.rs) — keep these in sync rather than adding
/// a fourth parse path.
pub(crate) fn parse_skills_config(
    config: &toml::Value,
) -> xai_grok_agent::prompt::skills::SkillsConfig {
    config
        .get("skills")
        .and_then(|v| v.clone().try_into().ok())
        .unwrap_or_default()
}

fn parse_compat_config(config: &toml::Value) -> xai_grok_tools::types::compat::CompatConfigToml {
    config
        .get("compat")
        .and_then(|v| v.clone().try_into().ok())
        .unwrap_or_default()
}

fn extract_ui_fields(config: &toml::Value) -> (Option<String>, bool, Option<String>) {
    let ui = config.get("ui").and_then(|v| v.as_table());
    let theme = ui
        .and_then(|u| u.get("theme"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let yolo = ui
        .and_then(|u| u.get("yolo"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let fork = ui
        .and_then(|u| u.get("fork_secondary_model"))
        .and_then(|v| v.as_str())
        .map(String::from);
    (theme, yolo, fork)
}

fn parse_compaction_config(
    config: &toml::Value,
) -> anyhow::Result<crate::agent::config::CompactionConfig> {
    let parsed = match config.get("compaction") {
        Some(value) => {
            let parsed: crate::agent::config::CompactionConfig = value
                .clone()
                .try_into()
                .map_err(|error| anyhow::anyhow!("could not deserialize [compaction]: {error}"))?;
            parsed
        }
        None => crate::agent::config::CompactionConfig::default(),
    };
    parsed
        .normalize_validate()
        .map_err(|error| anyhow::anyhow!("could not validate [compaction]: {error}"))?;
    Ok(parsed)
}

/// Hash the `[compaction]` section for change detection.
/// Uses the table's byte representation for stable hashing.
fn hash_compaction_config(config: &toml::Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    config
        .get("compaction")
        .map(|v| {
            let bytes = toml::to_string(v).unwrap_or_default();
            bytes.hash(&mut hasher);
        })
        .unwrap_or_else(|| {
            // No compaction section → hash of empty
            0u8.hash(&mut hasher);
        });
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::GrokAuth;
    use std::collections::BTreeMap;

    fn make_auth(key: &str) -> GrokAuth {
        GrokAuth {
            key: key.to_string(),
            email: Some("test@test.com".to_string()),
            ..GrokAuth::test_default()
        }
    }

    #[tokio::test]
    async fn reloader_skips_unchanged_auth() {
        let tmp = tempfile::TempDir::new().unwrap();
        let auth = make_auth("same-key");
        let mut store = BTreeMap::new();
        let scope = "https://test.example.com".to_string();
        store.insert(scope.clone(), auth);
        let json = serde_json::to_string_pretty(&store).unwrap();
        std::fs::write(tmp.path().join("auth.json"), &json).unwrap();

        let initial_hash = hash_auth_key("same-key");
        let (tx, mut rx) = mpsc::unbounded_channel();
        let empty_config = toml::Value::Table(toml::map::Map::new());
        let mut reloader = ConfigReloader::new(
            tmp.path().to_path_buf(),
            initial_hash,
            empty_config,
            scope,
            None,
            tx,
            false,
            false,
        );

        reloader.reload_auth().unwrap();
        assert!(
            rx.try_recv().is_err(),
            "should not send update when key is unchanged"
        );
    }

    #[tokio::test]
    async fn reloader_detects_new_auth_key() {
        let tmp = tempfile::TempDir::new().unwrap();
        let auth = make_auth("new-key");
        let mut store = BTreeMap::new();
        let scope = "https://test.example.com".to_string();
        store.insert(scope.clone(), auth);
        let json = serde_json::to_string_pretty(&store).unwrap();
        std::fs::write(tmp.path().join("auth.json"), &json).unwrap();

        let old_hash = hash_auth_key("old-key");
        let (tx, mut rx) = mpsc::unbounded_channel();
        let empty_config = toml::Value::Table(toml::map::Map::new());
        let mut reloader = ConfigReloader::new(
            tmp.path().to_path_buf(),
            old_hash,
            empty_config,
            scope,
            None,
            tx,
            false,
            false,
        );

        reloader.reload_auth().unwrap();
        let update = rx.try_recv().expect("should send Auth update");
        assert!(
            matches!(update, ConfigUpdate::Auth(a) if a.key == "new-key"), // a is Box<GrokAuth>, Deref coercion
            "should contain new key"
        );
    }

    #[tokio::test]
    async fn reloader_detects_auth_cleared() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Write auth.json with a DIFFERENT scope — our scope is missing
        let auth = make_auth("other-key");
        let mut store = BTreeMap::new();
        store.insert("https://other.example.com".to_string(), auth);
        let json = serde_json::to_string_pretty(&store).unwrap();
        std::fs::write(tmp.path().join("auth.json"), &json).unwrap();

        let old_hash = hash_auth_key("had-a-key");
        let (tx, mut rx) = mpsc::unbounded_channel();
        let empty_config = toml::Value::Table(toml::map::Map::new());
        let mut reloader = ConfigReloader::new(
            tmp.path().to_path_buf(),
            old_hash,
            empty_config,
            "https://test.example.com".to_string(),
            None,
            tx,
            false,
            false,
        );

        reloader.reload_auth().unwrap();
        let update = rx.try_recv().expect("should send AuthCleared");
        assert!(matches!(update, ConfigUpdate::AuthCleared));
    }

    #[tokio::test]
    async fn reloader_handles_malformed_auth_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("auth.json"), "not valid json{{{").unwrap();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let empty_config = toml::Value::Table(toml::map::Map::new());
        let mut reloader = ConfigReloader::new(
            tmp.path().to_path_buf(),
            0,
            empty_config,
            "https://test.example.com".to_string(),
            None,
            tx,
            false,
            false,
        );

        let result = reloader.reload_auth();
        assert!(result.is_err(), "malformed JSON should return error");
        assert!(
            rx.try_recv().is_err(),
            "should not send update on parse failure"
        );
    }

    #[tokio::test]
    async fn reloader_handles_missing_auth_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No auth.json written

        let (tx, mut rx) = mpsc::unbounded_channel();
        let empty_config = toml::Value::Table(toml::map::Map::new());
        let mut reloader = ConfigReloader::new(
            tmp.path().to_path_buf(),
            0,
            empty_config,
            "https://test.example.com".to_string(),
            None,
            tx,
            false,
            false,
        );

        let result = reloader.reload_auth();
        assert!(result.is_err(), "missing file should return error");
        assert!(
            rx.try_recv().is_err(),
            "should not send update on missing file"
        );
    }

    /// A `models_cache.json` rewrite is a pass-through the reloader cannot
    /// diff (TTL/version/auth validation needs `ModelsManager` state), so the
    /// event surfaces as a coalesced reload intent with the cache flag set.
    #[tokio::test]
    async fn reloader_forwards_models_cache_changed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let empty_config = toml::Value::Table(toml::map::Map::new());
        let reloader = ConfigReloader::new(
            tmp.path().to_path_buf(),
            0,
            empty_config,
            "https://test.example.com".to_string(),
            None,
            tx,
            false,
            false,
        );

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let cancel = CancellationToken::new();
        let handle = tokio::spawn(reloader.run(event_rx, cancel.clone()));

        event_tx
            .send(ConfigChangeEvent::ModelsCacheChanged)
            .unwrap();

        let update = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("should receive an update within 2s")
            .expect("update channel should remain open");
        assert!(
            matches!(
                update,
                ConfigUpdate::ModelReload {
                    models: false,
                    providers: false,
                    retrieval: false,
                    cache: true,
                }
            ),
            "cache-only event must coalesce into one reload intent: {update:?}"
        );

        cancel.cancel();
        let _ = handle.await;
    }

    /// Model-section diffing is deterministic and does not depend on the
    /// process-global GROK_HOME/OnceLock, so it is safe under both nextest and
    /// shared-process `cargo test`.
    #[test]
    fn reload_config_value_detects_model_change_without_global_home() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let initial = toml::Value::Table(toml::map::Map::new());
        let mut reloader = ConfigReloader::new(
            tmp.path().to_path_buf(),
            0,
            initial,
            "https://test.example.com".to_string(),
            None,
            tx,
            false,
            false,
        );
        let changed: toml::Value =
            toml::from_str("[models]\ndefault = \"grok-code-fast-1\"\n").unwrap();

        let diffs = reloader.reload_config_value(changed);

        assert!(diffs.models);
        assert!(!diffs.providers);
        assert!(!diffs.retrieval);
    }

    /// Provider changes affect both catalog publication and exact retrieval
    /// pins, so the serialized owner must validate/publish them as one batch.
    #[test]
    fn reload_config_value_marks_provider_change_as_retrieval_affecting() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let initial = toml::Value::Table(toml::map::Map::new());
        let mut reloader = ConfigReloader::new(
            tmp.path().to_path_buf(),
            0,
            initial,
            "https://test.example.com".to_string(),
            None,
            tx,
            false,
            false,
        );
        let changed: toml::Value = toml::from_str(
            "[model_providers.team]\nkind = \"openai_compatible\"\nbase_url = \"https://example.test/v1\"\napi_surfaces = [\"openai_compatible_subset\"]\ncredential_routes = [\"none\"]\n",
        )
        .unwrap();

        let diffs = reloader.reload_config_value(changed);

        assert!(diffs.models);
        assert!(diffs.providers);
        assert!(diffs.retrieval);
    }

    /// A project event with unchanged bytes must not re-dispatch a
    /// reload; the first event and a later real edit must both dispatch.
    #[tokio::test]
    async fn reloader_dedupes_unchanged_project_mcp_config() {
        let tmp = tempfile::TempDir::new().unwrap();
        git2::Repository::init(tmp.path()).unwrap();
        let cwd = tmp.path().to_path_buf();
        let mcp_json = cwd.join(".mcp.json");
        std::fs::write(&mcp_json, r#"{"mcpServers":{}}"#).unwrap();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let empty_config = toml::Value::Table(toml::map::Map::new());
        let reloader = ConfigReloader::new(
            tmp.path().to_path_buf(),
            0,
            empty_config,
            "https://test.example.com".to_string(),
            None,
            tx,
            false,
            false,
        );

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let cancel = CancellationToken::new();
        let handle = tokio::spawn(reloader.run(event_rx, cancel.clone()));

        let evt = || ConfigChangeEvent::McpConfigChanged {
            path: mcp_json.clone(),
        };

        // First event → dispatch (no prior hash for this cwd).
        event_tx.send(evt()).unwrap();
        let update = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("first event should dispatch within 2s")
            .expect("channel open");
        assert!(
            matches!(update, ConfigUpdate::ProjectMcpServersChanged { cwd: ref c } if *c == cwd),
            "first project event must dispatch"
        );

        // Second event, identical bytes → must be suppressed.
        event_tx.send(evt()).unwrap();
        let res = tokio::time::timeout(std::time::Duration::from_millis(400), rx.recv()).await;
        assert!(
            res.is_err(),
            "unchanged project config must not re-dispatch a reload"
        );

        // Real content change → dispatch again.
        std::fs::write(
            &mcp_json,
            r#"{"mcpServers":{"x":{"url":"http://localhost"}}}"#,
        )
        .unwrap();
        event_tx.send(evt()).unwrap();
        let update = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("changed content should dispatch within 2s")
            .expect("channel open");
        assert!(
            matches!(update, ConfigUpdate::ProjectMcpServersChanged { cwd: ref c } if *c == cwd),
            "changed project config must dispatch"
        );

        cancel.cancel();
        let _ = handle.await;
    }

    /// `hash_project_mcp_config` is stable for identical content and
    /// changes on create/edit.
    #[test]
    fn hash_project_mcp_config_detects_create_and_change() {
        let tmp = tempfile::TempDir::new().unwrap();
        git2::Repository::init(tmp.path()).unwrap();
        let cwd = tmp.path();

        let empty = hash_project_mcp_config(cwd).expect("readable");
        assert_eq!(empty, hash_project_mcp_config(cwd).expect("stable"));

        std::fs::write(cwd.join(".mcp.json"), "a").unwrap();
        let created = hash_project_mcp_config(cwd).expect("readable");
        assert_ne!(empty, created, "creating a config file changes the hash");

        std::fs::write(cwd.join(".mcp.json"), "b").unwrap();
        let changed = hash_project_mcp_config(cwd).expect("readable");
        assert_ne!(created, changed, "editing content changes the hash");
    }

    /// The hash must reflect ancestor `.grok/config.toml` and `.mcp.json`
    /// under `cwd` — otherwise an ancestor edit would be wrongly
    /// must be a distinct variant from the unit `McpServersChanged`
    /// suppressed.
    #[test]
    fn hash_project_mcp_config_covers_ancestors() {
        let tmp = tempfile::TempDir::new().unwrap();
        git2::Repository::init(tmp.path()).unwrap();
        let child = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&child).unwrap();

        let h0 = hash_project_mcp_config(&child).expect("readable");

        std::fs::write(tmp.path().join(".mcp.json"), "a").unwrap();
        let h1 = hash_project_mcp_config(&child).expect("readable");
        assert_ne!(h0, h1, "ancestor .mcp.json create must change the hash");

        std::fs::write(tmp.path().join(".mcp.json"), "b").unwrap();
        let h2 = hash_project_mcp_config(&child).expect("readable");
        assert_ne!(h1, h2, "ancestor .mcp.json edit must change the hash");

        std::fs::create_dir_all(tmp.path().join(".grok")).unwrap();
        std::fs::write(tmp.path().join(".grok").join("config.toml"), "x = 1").unwrap();
        let h3 = hash_project_mcp_config(&child).expect("readable");
        assert_ne!(
            h2, h3,
            "ancestor .grok/config.toml create must change the hash"
        );
    }

    #[test]
    fn compaction_reload_dedupes_last_accepted_value() {
        let tmp = tempfile::TempDir::new().unwrap();
        let initial = toml::Value::Table(toml::map::Map::new());
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut reloader = ConfigReloader::new(
            tmp.path().to_path_buf(),
            0,
            initial,
            "https://test.example.com".to_owned(),
            None,
            tx,
            false,
            false,
        );
        let rolling: toml::Value = toml::from_str(
            r#"
[compaction]
strategy = "rolling"
trigger_policy = "dynamic"
rolling_band_count = 6
models = ["@session", "custom:summarizer"]
"#,
        )
        .unwrap();

        reloader.maybe_reload_compaction(&rolling);
        let update = rx.try_recv().expect("changed valid config should dispatch");
        let ConfigUpdate::Compaction(config) = update else {
            panic!("expected compaction update");
        };
        let resolved = config.normalize_validate().unwrap();
        assert_eq!(
            resolved.strategy,
            crate::agent::config::CompactionStrategy::Rolling
        );
        assert_eq!(resolved.rolling_band_count, 6);
        assert_eq!(resolved.models[1].model_id(), "custom:summarizer");

        reloader.maybe_reload_compaction(&rolling);
        assert!(
            rx.try_recv().is_err(),
            "unchanged accepted config must not dispatch twice"
        );
    }

    #[test]
    fn invalid_compaction_reload_keeps_last_known_good() {
        let tmp = tempfile::TempDir::new().unwrap();
        let initial: toml::Value = toml::from_str("[compaction]\nstrategy = \"rolling\"").unwrap();
        let accepted_hash = hash_compaction_config(&initial);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut reloader = ConfigReloader::new(
            tmp.path().to_path_buf(),
            0,
            initial,
            "https://test.example.com".to_owned(),
            None,
            tx,
            false,
            false,
        );
        let invalid_enum: toml::Value =
            toml::from_str("[compaction]\nstrategy = \"unknown\"").unwrap();
        let invalid_band: toml::Value =
            toml::from_str("[compaction]\nrolling_band_count = 9").unwrap();

        reloader.maybe_reload_compaction(&invalid_enum);
        reloader.maybe_reload_compaction(&invalid_band);

        assert!(rx.try_recv().is_err(), "invalid config must not dispatch");
        assert_eq!(
            reloader.last_compaction_hash, accepted_hash,
            "invalid observations must not replace the accepted hash"
        );
    }

    #[test]
    fn parse_skills_config_empty() {
        let config = toml::Value::Table(toml::map::Map::new());
        let skills = parse_skills_config(&config);
        assert_eq!(
            skills,
            xai_grok_agent::prompt::skills::SkillsConfig::default()
        );
    }

    #[test]
    fn parse_skills_config_with_paths() {
        let config: toml::Value = toml::from_str(
            r#"
[skills]
paths = ["/home/user/.grok/skills"]
ignore = ["/tmp"]
"#,
        )
        .unwrap();
        let skills = parse_skills_config(&config);
        assert_eq!(skills.paths, vec!["/home/user/.grok/skills".to_string()]);
        assert_eq!(skills.ignore, vec!["/tmp".to_string()]);
    }

    #[test]
    fn memory_config_diff_detects_enabled_change() {
        let empty = toml::Value::Table(toml::map::Map::new());
        let enabled: toml::Value = toml::from_str("[memory]\nenabled = true").unwrap();

        let old = crate::config::MemoryConfig::resolve(false, false, &empty, None);
        let new = crate::config::MemoryConfig::resolve(false, false, &enabled, None);
        assert_ne!(old, new, "should detect enabled field change");
    }

    #[test]
    fn memory_config_diff_detects_search_param_change() {
        let a: toml::Value = toml::from_str("[memory.search]\nmax_results = 6").unwrap();
        let b: toml::Value = toml::from_str("[memory.search]\nmax_results = 10").unwrap();

        let old = crate::config::MemoryConfig::resolve(false, false, &a, None);
        let new = crate::config::MemoryConfig::resolve(false, false, &b, None);
        assert_ne!(old, new, "should detect search param change");
    }

    #[test]
    fn extract_ui_fields_empty() {
        let config = toml::Value::Table(toml::map::Map::new());
        let (theme, yolo, fork) = extract_ui_fields(&config);
        assert_eq!(theme, None);
        assert!(!yolo);
        assert_eq!(fork, None);
    }

    #[test]
    fn extract_ui_fields_with_values() {
        let config: toml::Value = toml::from_str(
            r#"
[ui]
theme = "dark"
yolo = true
fork_secondary_model = "grok-4.5"
"#,
        )
        .unwrap();
        let (theme, yolo, fork) = extract_ui_fields(&config);
        assert_eq!(theme.as_deref(), Some("dark"));
        assert!(yolo);
        assert_eq!(fork.as_deref(), Some("grok-4.5"));
    }

    #[test]
    fn extract_ui_fields_diff_detects_theme_change() {
        let a: toml::Value = toml::from_str("[ui]\ntheme = \"light\"").unwrap();
        let b: toml::Value = toml::from_str("[ui]\ntheme = \"dark\"").unwrap();
        assert_ne!(extract_ui_fields(&a), extract_ui_fields(&b));
    }

    #[test]
    fn extract_ui_fields_diff_detects_yolo_change() {
        let a: toml::Value = toml::from_str("[ui]\nyolo = false").unwrap();
        let b: toml::Value = toml::from_str("[ui]\nyolo = true").unwrap();
        assert_ne!(extract_ui_fields(&a), extract_ui_fields(&b));
    }

    #[test]
    fn models_changed_detects_new_byok_model() {
        let a = toml::Value::Table(toml::map::Map::new());
        let b: toml::Value = toml::from_str(
            r#"
[model.my-custom]
model = "grok-4.5"
base_url = "https://api.example.com/v1"
"#,
        )
        .unwrap();
        assert_ne!(a.get("model"), b.get("model"));
    }

    #[test]
    fn models_changed_detects_default_change() {
        let a: toml::Value = toml::from_str("[models]\ndefault = \"grok-code-fast-1\"").unwrap();
        let b: toml::Value = toml::from_str("[models]\ndefault = \"grok-code-slow-1\"").unwrap();
        assert_ne!(a.get("models"), b.get("models"));
    }

    #[test]
    fn mcp_servers_changed_detects_new_server() {
        let a = toml::Value::Table(toml::map::Map::new());
        let b: toml::Value = toml::from_str(
            r#"
[mcp_servers.test]
command = "/bin/test"
"#,
        )
        .unwrap();
        assert_ne!(a.get("mcp_servers"), b.get("mcp_servers"));
    }

    /// `ConfigUpdate::ProjectMcpServersChanged { cwd }`
    /// so the two paths route through different match arms in
    /// `app.rs`. Guards against an accidental merge that would force
    /// fan-out — it must NOT contribute a cwd to
    /// per-cwd reloads through the legacy sweep-all-sessions arm.
    #[test]
    fn project_variant_dispatches_separately() {
        let cwd = PathBuf::from("/tmp/proj-x");
        let global: ConfigUpdate = ConfigUpdate::McpServersChanged;
        let project = ConfigUpdate::ProjectMcpServersChanged { cwd: cwd.clone() };

        // Each variant must be matched by its own arm — fall-through
        // would indicate a single arm handling both.
        let mut routed_global = false;
        let mut routed_project = None;
        for u in [global, project] {
            match u {
                ConfigUpdate::McpServersChanged => routed_global = true,
                ConfigUpdate::ProjectMcpServersChanged { cwd } => routed_project = Some(cwd),
                _ => panic!("unexpected variant"),
            }
        }
        assert!(
            routed_global,
            "global variant must route through its own arm"
        );
        assert_eq!(routed_project.as_deref(), Some(cwd.as_path()));
    }

    /// `HomeClaudeJsonChanged` is **not** part of the per-cwd
    /// `collect_project_cwds` (otherwise sessions outside `$HOME`
    /// would be silently skipped). The reloader broadcasts it via
    /// the unit `McpServersChanged` variant; this test locks that
    /// `ProjectConfigChanged` (`<cwd>/.grok/config.toml`) and
    /// invariant at the helper layer.
    #[test]
    fn collect_project_cwds_excludes_home_claude_json() {
        let batch = vec![
            ConfigChangeEvent::HomeClaudeJsonChanged,
            ConfigChangeEvent::ProjectConfigChanged {
                path: PathBuf::from("/repo/x/.grok/config.toml"),
            },
        ];
        let cwds = collect_project_cwds(&batch);
        // Only the project entry contributes; the home-level `.claude.json`
        // entry is silently dropped because it routes through the
        // broadcast arm instead.
        assert_eq!(cwds, vec![PathBuf::from("/repo/x")]);
    }

    /// `collect_project_cwds` extracts `<cwd>` from
    /// `McpConfigChanged` (`<cwd>/.mcp.json`), de-duplicates while
    /// `McpConfigChanged` (`<cwd>/.mcp.json`), de-duplicates while
    /// preserving order.
    #[test]
    fn collect_project_cwds_dedupes_and_extracts() {
        let batch = vec![
            ConfigChangeEvent::ProjectConfigChanged {
                path: PathBuf::from("/repo/a/.grok/config.toml"),
            },
            ConfigChangeEvent::McpConfigChanged {
                path: PathBuf::from("/repo/a/.mcp.json"),
            },
            ConfigChangeEvent::ProjectConfigChanged {
                path: PathBuf::from("/repo/b/.grok/config.toml"),
            },
        ];
        let cwds = collect_project_cwds(&batch);
        assert_eq!(
            cwds,
            vec![PathBuf::from("/repo/a"), PathBuf::from("/repo/b")]
        );
    }

    #[test]
    fn mcp_servers_unchanged_same_config() {
        let cfg: toml::Value = toml::from_str(
            r#"
[mcp_servers.test]
command = "/bin/test"
"#,
        )
        .unwrap();
        assert_eq!(cfg.get("mcp_servers"), cfg.get("mcp_servers"));
    }
}
