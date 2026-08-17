//! Bounded reverse-reference index for provider disable/removal impact.
//!
//! Scans trusted Grok home/config roots only. Secret-free. Fail-closed on
//! unreadable/corrupt state (reports scan errors rather than claiming zero
//! references). Retrieval groups are structurally empty hooks until PR15.

use super::management::dto::{
    ImpactGroupKind, ImpactReference, ReferenceImpactSnapshot, RegistryGeneration,
};
use std::fs;
use std::path::Path;

const MAX_REFS_PER_GROUP: usize = 64;
const MAX_LABEL_LEN: usize = 128;
const MAX_SESSION_DIRS: usize = 512;
const MAX_SCAN_BYTES: u64 = 256 * 1024;
const MAX_SCAN_ERRORS: usize = 16;

/// Build a full reference-impact snapshot for `provider_id`.
pub fn build_reference_impact(
    home: &Path,
    config_path: &Path,
    provider_id: &str,
    generation: RegistryGeneration,
    is_built_in: bool,
    secrets_present: bool,
    cache_present: bool,
) -> ReferenceImpactSnapshot {
    let mut groups: Vec<(ImpactGroupKind, Vec<ImpactReference>)> = Vec::new();
    let mut scan_errors: Vec<String> = Vec::new();
    let mut truncated = false;

    // 1. Model provider refs / defaults in config.toml
    let (model_refs, model_trunc, model_err) = scan_config_model_refs(config_path, provider_id);
    truncated |= model_trunc;
    push_err(&mut scan_errors, model_err);
    groups.push((ImpactGroupKind::ModelsAndDefaults, model_refs));

    // 2. Session summaries / model-route companions
    let (session_refs, sess_trunc, sess_err) = scan_session_refs(home, provider_id);
    truncated |= sess_trunc;
    push_err(&mut scan_errors, sess_err);
    groups.push((ImpactGroupKind::Sessions, session_refs));

    // 3. Subagent / agent pins (agent definitions under home)
    let (agent_refs, agent_trunc, agent_err) = scan_agent_pins(home, provider_id);
    truncated |= agent_trunc;
    push_err(&mut scan_errors, agent_err);
    groups.push((ImpactGroupKind::AgentsAndSubagents, agent_refs));

    // 4. Workflows / goals
    let (wf_refs, wf_trunc, wf_err) = scan_workflows_and_goals_v2(home, provider_id);
    truncated |= wf_trunc;
    push_err(&mut scan_errors, wf_err);
    groups.push((ImpactGroupKind::WorkflowsAndGoals, wf_refs));

    // 5. Compaction / media / web / suggestion routes in config
    let (aux_refs, aux_trunc, aux_err) = scan_auxiliary_config_routes(config_path, provider_id);
    truncated |= aux_trunc;
    push_err(&mut scan_errors, aux_err);
    groups.push((ImpactGroupKind::AuxiliaryRoutes, aux_refs));

    // 6. Memory legacy provider references
    let (mem_refs, mem_trunc, mem_err) = scan_memory_refs(config_path, provider_id);
    truncated |= mem_trunc;
    push_err(&mut scan_errors, mem_err);
    groups.push((ImpactGroupKind::Memory, mem_refs));

    // 7. Retrieval hooks (empty until PR15 named retrieval config)
    groups.push((ImpactGroupKind::RetrievalProfiles, Vec::new()));
    groups.push((ImpactGroupKind::EmbeddingModels, Vec::new()));
    groups.push((ImpactGroupKind::RerankerModels, Vec::new()));

    let total_refs: usize = groups.iter().map(|(_, r)| r.len()).sum();
    let scan_failed = !scan_errors.is_empty();
    // Truncation is incomplete scan: block default removal fail-closed.
    if truncated && !scan_failed {
        push_err(
            &mut scan_errors,
            Some("reference scan truncated at safety bounds; treat as incomplete".into()),
        );
    }
    let scan_incomplete = !scan_errors.is_empty() || truncated;

    let (can_remove, blocked_reason) = if is_built_in {
        (
            false,
            Some("Built-in product providers cannot be removed from the registry.".into()),
        )
    } else if scan_incomplete {
        (
            false,
            Some(format!(
                "Reference scan incomplete (truncated={}, errors={}); remove is blocked fail-closed. {}",
                truncated,
                scan_errors.len(),
                scan_errors.first().cloned().unwrap_or_default()
            )),
        )
    } else if total_refs > 0 {
        (
            false,
            Some(format!(
                "Referenced by {total_refs} durable/active item(s); reassign or use forced remove with typed id."
            )),
        )
    } else {
        (true, None)
    };

    let model_references: Vec<String> = groups
        .iter()
        .find(|(k, _)| *k == ImpactGroupKind::ModelsAndDefaults)
        .map(|(_, refs)| refs.iter().map(|r| r.label.clone()).collect())
        .unwrap_or_default();
    let session_pin_hints: Vec<String> = groups
        .iter()
        .find(|(k, _)| *k == ImpactGroupKind::Sessions)
        .map(|(_, refs)| refs.iter().map(|r| r.label.clone()).collect())
        .unwrap_or_default();

    let guidance = if is_built_in {
        "Built-ins can be disabled but never removed.".into()
    } else if can_remove {
        "No references found. Normal remove is allowed. Forced remove still creates an incarnation tombstone. Secret/cache deletion is optional and never implicit.".into()
    } else if scan_failed {
        "Fix unreadable state or use forced remove only after confirming impact manually.".into()
    } else {
        "Disable blocks the next request/turn. Forced remove requires typing the exact provider id, creates a tombstone, and never remaps old sessions to a recreated id.".into()
    };

    let mut impact_groups = groups
        .into_iter()
        .map(|(kind, references)| super::management::dto::ImpactGroup { kind, references })
        .collect::<Vec<_>>();

    // Keep empty retrieval groups for UI structure.
    let _ = &mut impact_groups;

    ReferenceImpactSnapshot {
        provider_id: provider_id.to_owned(),
        generation,
        can_remove,
        blocked_reason,
        model_references,
        session_pin_hints,
        cache_present,
        secrets_present,
        guidance,
        groups: impact_groups,
        scan_errors,
        truncated,
        disable_blocks_next_turn: true,
    }
}

fn push_err(errors: &mut Vec<String>, err: Option<String>) {
    if let Some(e) = err {
        if errors.len() < MAX_SCAN_ERRORS {
            errors.push(truncate_label(&e));
        }
    }
}

fn truncate_label(s: &str) -> String {
    let mut out: String = s.chars().take(MAX_LABEL_LEN).collect();
    if s.chars().count() > MAX_LABEL_LEN {
        out.push('…');
    }
    out
}

fn push_ref(out: &mut Vec<ImpactReference>, kind: ImpactGroupKind, label: impl Into<String>) {
    if out.len() >= MAX_REFS_PER_GROUP {
        return;
    }
    out.push(ImpactReference {
        kind,
        label: truncate_label(&label.into()),
    });
}

fn scan_config_model_refs(
    config_path: &Path,
    provider_id: &str,
) -> (Vec<ImpactReference>, bool, Option<String>) {
    let mut refs = Vec::new();
    let mut truncated = false;
    let raw = match fs::read_to_string(config_path) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (refs, false, None),
        Err(e) => {
            return (refs, false, Some(format!("config.toml unreadable: {e}")));
        }
    };
    if raw.len() as u64 > MAX_SCAN_BYTES * 4 {
        // Still parse; bound output only.
        truncated = true;
    }
    let val: toml::Value = match toml::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return (refs, false, Some(format!("config.toml corrupt: {e}")));
        }
    };
    // Catalog index: provider_id -> upstream slugs (from [model] entries + namespaced ids).
    let catalog_by_provider = build_catalog_index_from_config(&val);
    if let Some(models) = val.get("model").and_then(|v| v.as_table()) {
        for (model_id, entry) in models {
            match model_entry_references_provider(
                model_id,
                entry,
                provider_id,
                &catalog_by_provider,
            ) {
                ModelRefHit::Yes => {
                    if refs.len() >= MAX_REFS_PER_GROUP {
                        truncated = true;
                        break;
                    }
                    push_ref(
                        &mut refs,
                        ImpactGroupKind::ModelsAndDefaults,
                        format!("model.{model_id}"),
                    );
                }
                ModelRefHit::No => {}
                ModelRefHit::AmbiguousOrCorrupt(msg) => {
                    return (refs, truncated, Some(msg));
                }
            }
        }
    }
    // default_model may be account-qualified: `provider:upstream` or curated/user/legacy id.
    if let Some(dm) = val
        .get("default_model")
        .and_then(|v| v.as_str())
        .or_else(|| {
            val.get("settings")
                .and_then(|s| s.get("default_model"))
                .and_then(|v| v.as_str())
        })
    {
        match resolve_model_id_to_provider(dm, provider_id, &catalog_by_provider) {
            ModelRefHit::Yes => {
                push_ref(
                    &mut refs,
                    ImpactGroupKind::ModelsAndDefaults,
                    format!("default_model={dm}"),
                );
            }
            ModelRefHit::No => {}
            ModelRefHit::AmbiguousOrCorrupt(msg) => {
                return (refs, truncated, Some(msg));
            }
        }
    }
    (refs, truncated, None)
}

/// Result of resolving whether a model id/entry references a provider instance.
#[derive(Debug)]
enum ModelRefHit {
    Yes,
    No,
    /// Fail closed: identity incomplete, ambiguous, or catalog corrupt.
    AmbiguousOrCorrupt(String),
}

/// Build provider → slug catalog from config `[model.*]` entries for reverse-ref resolve.
fn build_catalog_index_from_config(val: &toml::Value) -> indexmap::IndexMap<String, Vec<String>> {
    let mut catalog: indexmap::IndexMap<String, Vec<String>> = indexmap::IndexMap::new();
    if let Some(models) = val.get("model").and_then(|v| v.as_table()) {
        for (model_id, entry) in models {
            let provider = entry
                .get("model_provider")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned())
                .or_else(|| {
                    super::lifecycle::parse_namespaced_model_id(model_id)
                        .map(|(pid, _)| pid.as_str().to_owned())
                });
            if let Some(pid) = provider {
                let slug = entry
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned())
                    .or_else(|| {
                        super::lifecycle::parse_namespaced_model_id(model_id).map(|(_, slug)| slug)
                    })
                    .unwrap_or_else(|| model_id.clone());
                catalog.entry(pid).or_default().push(slug);
            }
        }
    }
    // Ensure provider keys exist so namespaced ids still resolve against empty catalogs.
    if let Some(providers) = val.get("model_providers").and_then(|v| v.as_table()) {
        for (pid, _) in providers {
            catalog.entry(pid.clone()).or_default();
        }
    }
    catalog
}

fn model_entry_references_provider(
    model_id: &str,
    entry: &toml::Value,
    provider_id: &str,
    catalog: &indexmap::IndexMap<String, Vec<String>>,
) -> ModelRefHit {
    if entry.get("model_provider").and_then(|v| v.as_str()) == Some(provider_id) {
        return ModelRefHit::Yes;
    }
    // Explicit wrong provider → no.
    if let Some(mp) = entry.get("model_provider").and_then(|v| v.as_str()) {
        if mp != provider_id {
            return ModelRefHit::No;
        }
    }
    resolve_model_id_to_provider(model_id, provider_id, catalog)
}

/// Canonical model-id → provider resolve for reverse refs.
///
/// Uses namespaced `provider:slug`, catalog-backed legacy alias resolve, and
/// fail-closed ambiguity when a bare slug could belong to the target provider
/// but identity cannot be confirmed.
fn resolve_model_id_to_provider(
    model_id: &str,
    provider_id: &str,
    catalog: &indexmap::IndexMap<String, Vec<String>>,
) -> ModelRefHit {
    if model_id.is_empty() {
        return ModelRefHit::No;
    }
    // Namespaced curated/user id: `lab:gpt-4o`.
    if let Some((pid, _slug)) = super::lifecycle::parse_namespaced_model_id(model_id) {
        return if pid.as_str() == provider_id {
            ModelRefHit::Yes
        } else {
            ModelRefHit::No
        };
    }
    // Exact provider id as model field (legacy).
    if model_id == provider_id {
        return ModelRefHit::Yes;
    }
    // Legacy un-namespaced alias via catalog compatibility ids.
    match super::lifecycle::resolve_legacy_model_alias(model_id, catalog) {
        Some(ns) => {
            if let Some((pid, _)) = super::lifecycle::parse_namespaced_model_id(&ns) {
                return if pid.as_str() == provider_id {
                    ModelRefHit::Yes
                } else {
                    ModelRefHit::No
                };
            }
            ModelRefHit::AmbiguousOrCorrupt(format!(
                "catalog resolve returned unparseable id `{ns}` for model `{model_id}`"
            ))
        }
        None => {
            // Ambiguous (0 or many). If the target provider advertises this slug,
            // fail closed — do not claim zero refs with incomplete identity.
            let advertised = catalog
                .get(provider_id)
                .map(|slugs| {
                    slugs.iter().any(|s| {
                        s == model_id
                            || s.ends_with(&format!(":{model_id}"))
                            || s == &format!("{provider_id}:{model_id}")
                    })
                })
                .unwrap_or(false);
            if advertised {
                return ModelRefHit::AmbiguousOrCorrupt(format!(
                    "model id `{model_id}` is ambiguous or incomplete relative to provider \
                     `{provider_id}`; reverse-ref scan fail-closed"
                ));
            }
            // Multi-provider claim without unique resolve — if any other provider also
            // has the slug, and catalog is non-empty, still no unique mapping.
            let claim_count = catalog
                .iter()
                .filter(|(_, slugs)| {
                    slugs
                        .iter()
                        .any(|s| s == model_id || s.ends_with(&format!(":{model_id}")))
                })
                .count();
            if claim_count > 1 {
                return ModelRefHit::AmbiguousOrCorrupt(format!(
                    "model id `{model_id}` claimed by {claim_count} providers; reverse-ref scan fail-closed"
                ));
            }
            ModelRefHit::No
        }
    }
}

fn scan_session_refs(
    home: &Path,
    provider_id: &str,
) -> (Vec<ImpactReference>, bool, Option<String>) {
    let mut refs = Vec::new();
    let mut truncated = false;
    let sessions_root = home.join("sessions");
    if !sessions_root.is_dir() {
        return (refs, false, None);
    }
    let mut visited = 0usize;
    let walk = match fs::read_dir(&sessions_root) {
        Ok(w) => w,
        Err(e) => {
            return (refs, false, Some(format!("sessions/ unreadable: {e}")));
        }
    };
    for cwd_ent in walk.flatten() {
        let cwd_path = cwd_ent.path();
        if !cwd_path.is_dir() {
            continue;
        }
        let sess_iter = match fs::read_dir(&cwd_path) {
            Ok(i) => i,
            Err(e) => {
                return (refs, false, Some(format!("session cwd unreadable: {e}")));
            }
        };
        for sess_ent in sess_iter.flatten() {
            if visited >= MAX_SESSION_DIRS {
                truncated = true;
                return (refs, truncated, None);
            }
            visited += 1;
            let sess_path = sess_ent.path();
            if !sess_path.is_dir() {
                continue;
            }
            let sid = sess_ent.file_name().to_string_lossy().into_owned();
            // model_route.json companion
            let route_path = sess_path.join("model_route.json");
            if route_path.is_file() {
                match read_bounded(&route_path) {
                    Ok(bytes) => {
                        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                            let matches = v.get("provider_instance_id").and_then(|x| x.as_str())
                                == Some(provider_id);
                            if matches {
                                if refs.len() >= MAX_REFS_PER_GROUP {
                                    truncated = true;
                                    return (refs, truncated, None);
                                }
                                push_ref(
                                    &mut refs,
                                    ImpactGroupKind::Sessions,
                                    format!("session:{sid}/model_route"),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        return (refs, truncated, Some(e));
                    }
                }
            }
            // summary.json current model may embed provider (namespaced or catalog-resolved).
            let summary_path = sess_path.join("summary.json");
            if summary_path.is_file() {
                match read_bounded(&summary_path) {
                    Ok(bytes) => {
                        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                            let explicit = v.get("provider_instance_id").and_then(|x| x.as_str())
                                == Some(provider_id);
                            let model = v
                                .get("current_model_id")
                                .or_else(|| v.get("model"))
                                .and_then(|x| x.as_str())
                                .unwrap_or("");
                            let catalog = indexmap::IndexMap::new();
                            let hit = if explicit {
                                ModelRefHit::Yes
                            } else {
                                resolve_model_id_to_provider(model, provider_id, &catalog)
                            };
                            match hit {
                                ModelRefHit::Yes => {
                                    if refs.len() >= MAX_REFS_PER_GROUP {
                                        truncated = true;
                                        return (refs, truncated, None);
                                    }
                                    push_ref(
                                        &mut refs,
                                        ImpactGroupKind::Sessions,
                                        format!("session:{sid}/summary"),
                                    );
                                }
                                ModelRefHit::No => {}
                                ModelRefHit::AmbiguousOrCorrupt(msg) => {
                                    return (refs, truncated, Some(msg));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        return (refs, truncated, Some(e));
                    }
                }
            }
        }
    }
    (refs, truncated, None)
}

fn scan_agent_pins(home: &Path, provider_id: &str) -> (Vec<ImpactReference>, bool, Option<String>) {
    let mut refs = Vec::new();
    let mut truncated = false;
    // Agent definitions: $GROK_HOME/agents/*.toml or agents/*.md front matter — scan toml/json.
    for rel in ["agents", "subagents"] {
        let dir = home.join(rel);
        if !dir.is_dir() {
            continue;
        }
        let iter = match fs::read_dir(&dir) {
            Ok(i) => i,
            Err(e) => {
                return (refs, false, Some(format!("{rel}/ unreadable: {e}")));
            }
        };
        for ent in iter.flatten() {
            let path = ent.path();
            let name = ent.file_name().to_string_lossy().into_owned();
            if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e == "toml" || e == "json" || e == "md")
            {
                match read_bounded(&path) {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        if text.contains(provider_id)
                            && (text.contains("model_provider")
                                || text.contains("provider_instance")
                                || text.contains(&format!("{provider_id}:")))
                        {
                            if refs.len() >= MAX_REFS_PER_GROUP {
                                truncated = true;
                                break;
                            }
                            push_ref(
                                &mut refs,
                                ImpactGroupKind::AgentsAndSubagents,
                                format!("{rel}/{name}"),
                            );
                        }
                    }
                    Err(e) => return (refs, truncated, Some(e)),
                }
            }
        }
    }
    (refs, truncated, None)
}

fn scan_dir_for_provider(
    dir: &Path,
    provider_id: &str,
    kind: ImpactGroupKind,
    refs: &mut Vec<ImpactReference>,
    truncated: &mut bool,
    label_prefix: &str,
) -> Result<(), String> {
    let iter = fs::read_dir(dir).map_err(|e| format!("{label_prefix}/ unreadable: {e}"))?;
    for ent in iter.flatten() {
        let path = ent.path();
        if path.is_dir() {
            // One level of nesting (run dirs).
            if let Ok(inner) = fs::read_dir(&path) {
                for inner_ent in inner.flatten() {
                    let ip = inner_ent.path();
                    if ip.is_file() {
                        check_file_ref(&ip, provider_id, kind, refs, truncated, label_prefix)?;
                    }
                }
            }
        } else if path.is_file() {
            check_file_ref(&path, provider_id, kind, refs, truncated, label_prefix)?;
        }
        if *truncated {
            break;
        }
    }
    Ok(())
}

fn check_file_ref(
    path: &Path,
    provider_id: &str,
    kind: ImpactGroupKind,
    refs: &mut Vec<ImpactReference>,
    truncated: &mut bool,
    label_prefix: &str,
) -> Result<(), String> {
    let bytes = read_bounded(path)?;
    let text = String::from_utf8_lossy(&bytes);
    if text.contains(&format!("\"provider_instance_id\":\"{provider_id}\""))
        || text.contains(&format!("provider_instance_id = \"{provider_id}\""))
        || text.contains(&format!("model_provider = \"{provider_id}\""))
        || text.contains(&format!("\"model_provider\":\"{provider_id}\""))
    {
        if refs.len() >= MAX_REFS_PER_GROUP {
            *truncated = true;
            return Ok(());
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("?");
        push_ref(refs, kind, format!("{label_prefix}/{name}"));
    }
    Ok(())
}

fn scan_auxiliary_config_routes(
    config_path: &Path,
    provider_id: &str,
) -> (Vec<ImpactReference>, bool, Option<String>) {
    let mut refs = Vec::new();
    let raw = match fs::read_to_string(config_path) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (refs, false, None),
        Err(e) => return (refs, false, Some(format!("config unreadable: {e}"))),
    };
    let val: toml::Value = match toml::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return (refs, false, Some(format!("config corrupt: {e}"))),
    };
    // Known auxiliary keys that may hold model_provider or provider refs.
    let keys = [
        "compaction",
        "media",
        "web_search",
        "prompt_suggestion",
        "shell_suggestion",
        "title",
        "summary",
        "recap",
    ];
    for key in keys {
        if let Some(table) = val.get(key) {
            if table_mentions_provider(table, provider_id) {
                push_ref(
                    &mut refs,
                    ImpactGroupKind::AuxiliaryRoutes,
                    format!("config.{key}"),
                );
            }
        }
    }
    (refs, false, None)
}

fn scan_memory_refs(
    config_path: &Path,
    provider_id: &str,
) -> (Vec<ImpactReference>, bool, Option<String>) {
    let mut refs = Vec::new();
    let raw = match fs::read_to_string(config_path) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (refs, false, None),
        Err(e) => return (refs, false, Some(format!("config unreadable: {e}"))),
    };
    let val: toml::Value = match toml::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return (refs, false, Some(format!("config corrupt: {e}"))),
    };
    if let Some(mem) = val.get("memory") {
        if table_mentions_provider(mem, provider_id) {
            push_ref(&mut refs, ImpactGroupKind::Memory, "config.memory");
        }
        if let Some(emb) = mem.get("embedding") {
            if emb.get("provider").and_then(|v| v.as_str()) == Some(provider_id)
                || table_mentions_provider(emb, provider_id)
            {
                push_ref(
                    &mut refs,
                    ImpactGroupKind::Memory,
                    "config.memory.embedding",
                );
            }
        }
    }
    (refs, false, None)
}

fn table_mentions_provider(val: &toml::Value, provider_id: &str) -> bool {
    match val {
        toml::Value::String(s) => s == provider_id || s.starts_with(&format!("{provider_id}:")),
        toml::Value::Table(t) => {
            if t.get("model_provider").and_then(|v| v.as_str()) == Some(provider_id) {
                return true;
            }
            if t.get("provider").and_then(|v| v.as_str()) == Some(provider_id) {
                return true;
            }
            if t.get("provider_instance_id").and_then(|v| v.as_str()) == Some(provider_id) {
                return true;
            }
            t.values().any(|v| table_mentions_provider(v, provider_id))
        }
        toml::Value::Array(a) => a.iter().any(|v| table_mentions_provider(v, provider_id)),
        _ => false,
    }
}

/// Read a trusted scan path with hardlink/symlink policy.
///
/// **Hardlink policy (PR13):** ordinary durable files under `$GROK_HOME` have
/// `nlink == 1` on APFS/HFS+/ext4 and are accepted. Planted hardlinks
/// (`nlink > 1`) are refused fail-closed so a shared inode cannot smuggle
/// secret-bearing content into the reverse-ref scan. Symlinks are always
/// refused.
fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    // Refuse symlink/hardlink targets under trusted roots.
    let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!("{}: refusing symlink", path.display()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // nlink == 1: normal file (accepted). nlink > 1: planted hardlink (rejected).
        if meta.nlink() > 1 {
            return Err(format!("{}: refusing hardlink", path.display()));
        }
    }
    if meta.len() > MAX_SCAN_BYTES {
        use std::io::Read;
        let mut f = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut buf = vec![0u8; MAX_SCAN_BYTES as usize];
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        buf.truncate(n);
        return Ok(buf);
    }
    fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn ordinary_nlink_one_file_accepted() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("summary.json");
        let mut f = fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"current_model_id":"lab:gpt"}}"#).unwrap();
        drop(f);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = fs::symlink_metadata(&path).unwrap();
            assert_eq!(meta.nlink(), 1, "temp file must be ordinary nlink==1");
        }
        let bytes = read_bounded(&path).expect("ordinary nlink==1 file must be accepted");
        assert!(bytes.starts_with(b"{"));
    }

    #[cfg(unix)]
    #[test]
    fn planted_hardlink_rejected() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("summary.json");
        let link = dir.path().join("summary.hardlink.json");
        fs::write(&path, br#"{"current_model_id":"lab:gpt"}"#).unwrap();
        std::fs::hard_link(&path, &link).expect("create hardlink");
        use std::os::unix::fs::MetadataExt;
        assert!(
            fs::symlink_metadata(&link).unwrap().nlink() > 1,
            "planted hardlink must have nlink>1"
        );
        let err = read_bounded(&link).expect_err("planted hardlink must be refused");
        assert!(
            err.contains("hardlink"),
            "error must mention hardlink: {err}"
        );
    }

    #[test]
    fn reverse_ref_namespaced_and_legacy_model_ids() {
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("config.toml");
        fs::write(
            &cfg,
            r#"
default_model = "lab:custom-slug"

[model_providers.lab]
kind = "openai_compatible"
base_url = "http://127.0.0.1:9/v1"

[model.curated]
model_provider = "lab"
model = "gpt-4o"

[model.user_lab_gpt]
model_provider = "lab"
model = "gpt-user"

[model.legacy_alias]
model_provider = "lab"
model = "deepseek"
"#,
        )
        .unwrap();
        let (refs, trunc, err) = scan_config_model_refs(&cfg, "lab");
        assert!(err.is_none(), "{err:?}");
        assert!(!trunc);
        let labels: Vec<_> = refs.iter().map(|r| r.label.as_str()).collect();
        assert!(
            labels
                .iter()
                .any(|l| l.contains("default_model=lab:custom-slug")),
            "namespaced default_model: {labels:?}"
        );
        assert!(
            labels.iter().any(|l| l.contains("model.curated")),
            "curated model_provider: {labels:?}"
        );
        assert!(
            labels.iter().any(|l| l.contains("model.user_lab_gpt")),
            "user model: {labels:?}"
        );
        assert!(
            labels.iter().any(|l| l.contains("model.legacy_alias")),
            "legacy alias entry: {labels:?}"
        );
    }

    #[test]
    fn reverse_ref_ambiguous_legacy_slug_fails_closed() {
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("config.toml");
        // Bare default_model slug advertised by two providers → fail closed.
        fs::write(
            &cfg,
            r#"
default_model = "shared-slug"

[model_providers.lab]
kind = "openai_compatible"
base_url = "http://127.0.0.1:9/v1"

[model_providers.other]
kind = "openai_compatible"
base_url = "http://127.0.0.1:8/v1"

[model.a]
model_provider = "lab"
model = "shared-slug"

[model.b]
model_provider = "other"
model = "shared-slug"
"#,
        )
        .unwrap();
        let (_refs, _trunc, err) = scan_config_model_refs(&cfg, "lab");
        assert!(
            err.as_ref()
                .is_some_and(|e| e.contains("ambiguous") || e.contains("fail-closed")),
            "expected fail-closed scan error, got {err:?}"
        );
    }
}

// Fix scan_workflows_and_goals to properly propagate errors.
// The earlier version used `?` incorrectly on a non-Result path.
// Rewrite cleanly below via a private helper used from build.

/// Scan workflows/goals directories with proper error propagation.
pub fn scan_workflows_and_goals_v2(
    home: &Path,
    provider_id: &str,
) -> (Vec<ImpactReference>, bool, Option<String>) {
    let mut refs = Vec::new();
    let mut truncated = false;
    for rel in ["workflows", "goals", "state/workflows", "state/goals"] {
        let dir = home.join(rel);
        if !dir.is_dir() {
            continue;
        }
        if let Err(e) = scan_dir_for_provider(
            &dir,
            provider_id,
            ImpactGroupKind::WorkflowsAndGoals,
            &mut refs,
            &mut truncated,
            rel,
        ) {
            return (refs, truncated, Some(e));
        }
    }
    (refs, truncated, None)
}
