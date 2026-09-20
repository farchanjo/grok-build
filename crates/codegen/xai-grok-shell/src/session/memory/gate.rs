//! Session glue for the memory write gate.
//!
//! The engine (questions, thresholds, store, fail-open outcomes) lives in
//! `xai_grok_memory::gate`. This module owns what is session-specific:
//! resolving `[memory.gate]`, building the transport with the profile's
//! OpenRouter credential, reading the current memory entries into a store, and
//! performing the append once the gate has ruled.
//!
//! Everything fails open. With `[memory.gate] enabled = false`, with no
//! credential, or on any transport/parse failure, the note is appended exactly
//! as it was before the gate existed.

use std::path::Path;
use std::sync::Arc;

use toml::Value as TomlValue;

use super::{GateOutcome, MemoryScope, MemoryStorage, note_scope};
use crate::config::MemoryGateConfig;
use xai_grok_memory::gate::{
    Candidate, DEFAULT_ENDPOINT, DEFAULT_ENTRY_CHARS, DEFAULT_MODEL, DEFAULT_STORE_MAX_CHARS,
    DEFAULT_STORE_MAX_ENTRIES, DEFAULT_TIMEOUT_MS, DropReason, GateConfig, MemoryGate, Store,
    WriteScope,
};
use xai_grok_memory::gate_client::JevDecisionsClient;

/// What happened to one note offered to the append seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteOutcome {
    /// Written to `scope`.
    Stored { scope: MemoryScope },
    /// The gate ruled the note out.
    Dropped { reason: DropReason },
    /// The gate could not rule; the note was written as before.
    FailOpen { scope: MemoryScope, error: String },
}

/// Resolve the gate policy from `[memory.gate]`.
pub fn resolve_config(raw: &MemoryGateConfig) -> GateConfig {
    let blank = |value: &Option<String>| {
        value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    GateConfig {
        enabled: raw.enabled,
        rerank: raw.rerank,
        endpoint: blank(&raw.endpoint).unwrap_or_else(|| DEFAULT_ENDPOINT.to_owned()),
        model: blank(&raw.model).unwrap_or_else(|| DEFAULT_MODEL.to_owned()),
        api_key_env: blank(&raw.api_key_env),
        zdr: raw.zdr,
        data_collection: blank(&raw.data_collection),
        require_parameters: raw.require_parameters,
        worth_threshold: raw.worth_threshold,
        covered_threshold: raw.covered_threshold,
        timeout_ms: raw
            .timeout_ms
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_TIMEOUT_MS),
        store_max_entries: raw
            .store_max_entries
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_STORE_MAX_ENTRIES),
        store_max_chars: raw
            .store_max_chars
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_STORE_MAX_CHARS),
        entry_chars: DEFAULT_ENTRY_CHARS,
    }
}

/// Read `[memory.gate]` from the user config file.
///
/// Read per call rather than cached: the note path is user-initiated and rare,
/// and a cache would go stale the moment `/memory-gate` writes the file.
/// Project-layer `.grok/config.toml` overrides are not consulted here.
pub fn config_from_user_file() -> GateConfig {
    resolve_config(&gate_table().unwrap_or_default())
}

/// The raw `[memory.gate]` table, or `None` when absent or unreadable.
fn gate_table() -> Option<MemoryGateConfig> {
    let text = std::fs::read_to_string(crate::util::config::user_config_path()).ok()?;
    let root: TomlValue = toml::from_str(&text).ok()?;
    root.get("memory")?.get("gate")?.clone().try_into().ok()
}

/// Build a gate with the OpenRouter credential, or `None` when there is none.
pub fn build_gate(config: GateConfig) -> Option<MemoryGate> {
    let stored = crate::auth::read_provider_api_key(
        &crate::util::grok_home::grok_home(),
        crate::auth::OPENROUTER_API_KEY_SCOPE,
    )
    .ok()
    .flatten();
    let client = JevDecisionsClient::new(&config, stored.as_deref()).ok()?;
    Some(MemoryGate::new(Arc::new(client), config))
}

/// The append seam: rule on one note, then write it where the gate says.
///
/// The error is the append itself failing; a gate failure is an `Ok` carrying
/// [`NoteOutcome::FailOpen`], because the note was still written.
pub async fn admit_note(cwd: &Path, text: &str) -> std::io::Result<NoteOutcome> {
    let storage = MemoryStorage::new(cwd, None);
    let config = config_from_user_file();
    let gate = config
        .is_enabled()
        .then(|| build_gate(config.clone()))
        .flatten();
    let Some(gate) = gate else {
        return append_plain(&storage, cwd, text, None);
    };
    let store = Store::build(storage.existing_entries(), &config);
    let workspace_path = cwd.to_string_lossy().to_string();
    let candidate = Candidate {
        text,
        workspace_path: &workspace_path,
    };
    match gate.decide(&candidate, &store).await {
        GateOutcome::Stored { scope, .. } => {
            let scope = write_scope(scope, &storage);
            storage.append_to_memory(scope, text)?;
            Ok(NoteOutcome::Stored { scope })
        }
        GateOutcome::Dropped { reason, .. } => Ok(NoteOutcome::Dropped { reason }),
        GateOutcome::Failed { error } => append_plain(&storage, cwd, text, Some(error)),
        GateOutcome::Disabled => append_plain(&storage, cwd, text, None),
    }
}

/// Write exactly as the path did before the gate existed.
fn append_plain(
    storage: &MemoryStorage,
    cwd: &Path,
    text: &str,
    error: Option<String>,
) -> std::io::Result<NoteOutcome> {
    let scope = note_scope(cwd);
    storage.append_to_memory(scope, text)?;
    match error {
        Some(error) => Ok(NoteOutcome::FailOpen { scope, error }),
        None => Ok(NoteOutcome::Stored { scope }),
    }
}

/// Drop the dream sections memory already covers, reusing the `covered`
/// question. `None` keeps the response untouched.
pub async fn consolidate_covered(storage: &MemoryStorage, response: &str) -> Option<String> {
    let config = config_from_user_file();
    if !config.is_enabled() {
        return None;
    }
    let gate = build_gate(config.clone())?;
    let sections = super::storage::markdown_sections(response);
    if sections.len() < 2 {
        return None;
    }
    let store = Store::build(storage.existing_entries(), &config);
    let kept = gate.covered_only(&sections, &store).await?;
    (kept.len() != sections.len()).then(|| kept.join("\n\n"))
}

/// Map a gate scope onto the storage scope, honouring an ephemeral cwd.
fn write_scope(scope: WriteScope, storage: &MemoryStorage) -> MemoryScope {
    match scope {
        WriteScope::ThisFolder if !storage.is_ephemeral() => MemoryScope::Workspace,
        _ => MemoryScope::Global,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(enabled: bool) -> MemoryGateConfig {
        MemoryGateConfig {
            enabled,
            ..Default::default()
        }
    }

    #[test]
    fn blank_strings_fall_back_to_defaults() {
        let mut config = raw(true);
        config.model = Some("   ".to_owned());
        config.endpoint = Some(String::new());
        let resolved = resolve_config(&config);
        assert_eq!(resolved.model, DEFAULT_MODEL);
        assert_eq!(resolved.endpoint, DEFAULT_ENDPOINT);
        assert_eq!(resolved.timeout_ms, DEFAULT_TIMEOUT_MS);
    }

    #[test]
    fn thresholds_keep_the_measured_defaults() {
        let resolved = resolve_config(&raw(false));
        assert_eq!(resolved.worth_threshold, 0.20);
        assert_eq!(resolved.covered_threshold, 0.50);
        assert!(!resolved.is_enabled());
    }

    #[test]
    fn zero_knobs_fall_back_to_defaults() {
        let mut config = raw(true);
        config.timeout_ms = Some(0);
        config.store_max_entries = Some(0);
        config.store_max_chars = Some(0);
        let resolved = resolve_config(&config);
        assert_eq!(resolved.timeout_ms, DEFAULT_TIMEOUT_MS);
        assert_eq!(resolved.store_max_entries, DEFAULT_STORE_MAX_ENTRIES);
        assert_eq!(resolved.store_max_chars, DEFAULT_STORE_MAX_CHARS);
    }

    #[test]
    fn ephemeral_folder_never_routes_to_workspace() {
        let storage = MemoryStorage::with_paths(
            std::path::PathBuf::from("/mem"),
            std::path::PathBuf::from("/mem/ws"),
        );
        assert_eq!(
            write_scope(WriteScope::ThisFolder, &storage),
            MemoryScope::Workspace
        );
        assert_eq!(
            write_scope(WriteScope::Global, &storage),
            MemoryScope::Global
        );
    }
}
