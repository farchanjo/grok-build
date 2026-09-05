//! Comment-preserving atomic TOML writes for the retrieval graph.
//!
//! Replaces only the managed sections (`embedding_models`, `reranker_models`,
//! `retrieval_profiles`, `prime`, and `memory.retrieval_profile`). Unknown
//! fields on individual entries, comments, ordering of unrelated sections, and
//! all other config content are preserved where `toml_edit` supports it.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use indexmap::IndexMap;
use xai_grok_config_types::{
    EmbeddingModelConfig, PrimeConfig, RerankerModelConfig, RetrievalGraphConfig,
    RetrievalProfileConfig,
};

/// Apply the full graph to `config.toml` with one atomic write.
pub fn write_retrieval_graph(
    config_path: &Path,
    graph: &RetrievalGraphConfig,
) -> Result<(), String> {
    let mut doc = read_document(config_path)?;
    apply_graph_to_document(&mut doc, graph);
    atomic_write_document(config_path, &doc).map_err(|e| format!("write config: {e}"))
}

fn read_document(path: &Path) -> Result<toml_edit::DocumentMut, String> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("read config: {e}")),
    };
    text.parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("parse config: {e}"))
}

fn apply_graph_to_document(doc: &mut toml_edit::DocumentMut, graph: &RetrievalGraphConfig) {
    // embedding_models
    write_map_section(
        doc,
        "embedding_models",
        &graph.embedding_models,
        write_embedding_table,
        EMBEDDING_KNOWN_KEYS,
    );
    // reranker_models
    write_map_section(
        doc,
        "reranker_models",
        &graph.reranker_models,
        write_reranker_table,
        RERANKER_KNOWN_KEYS,
    );
    // retrieval_profiles
    write_map_section(
        doc,
        "retrieval_profiles",
        &graph.retrieval_profiles,
        write_profile_table,
        PROFILE_KNOWN_KEYS,
    );
    // prime
    write_prime(doc, &graph.prime);
    // memory section
    write_memory_section(doc, graph);
}

/// Known keys for embedding model tables (optional None values are cleared).
const EMBEDDING_KNOWN_KEYS: &[&str] = &[
    "provider",
    "model",
    "protocol",
    "dimensions",
    "encoding",
    "batch_size",
    "max_input_tokens",
];
/// Known keys for reranker model tables.
const RERANKER_KNOWN_KEYS: &[&str] = &[
    "provider",
    "model",
    "protocol",
    "endpoint",
    "batch_size",
    "max_input_tokens",
];
/// Known keys for retrieval profile tables.
const PROFILE_KNOWN_KEYS: &[&str] = &[
    "embedding_models",
    "reranker_models",
    "fallback_strategy",
    "max_candidates",
    "max_results",
    "min_score",
    "deadline_ms",
    "max_attempts",
    "max_input_tokens",
    "max_output_tokens",
];

fn write_map_section<T>(
    doc: &mut toml_edit::DocumentMut,
    key: &str,
    map: &IndexMap<String, T>,
    write_entry: fn(&T) -> toml_edit::Table,
    known_keys: &[&str],
) {
    if map.is_empty() {
        // Remove empty section entirely so we do not leave a hollow table.
        doc.as_table_mut().remove(key);
        return;
    }
    let mut section = toml_edit::Table::new();
    section.set_implicit(true);
    for (id, cfg) in map {
        let mut entry = write_entry(cfg);
        // Preserve only *unknown* keys already on disk. Known optional fields
        // omitted as None must stay cleared (not resurrected from disk).
        if let Some(existing) = doc
            .get(key)
            .and_then(|i| i.as_table())
            .and_then(|t| t.get(id))
            .and_then(|i| i.as_table())
        {
            for (k, v) in existing.iter() {
                if !known_keys.contains(&k) && !entry.contains_key(k) {
                    entry.insert(k, v.clone());
                }
            }
        }
        section.insert(id, toml_edit::Item::Table(entry));
    }
    doc[key] = toml_edit::Item::Table(section);
}

fn write_embedding_table(cfg: &EmbeddingModelConfig) -> toml_edit::Table {
    let mut t = toml_edit::Table::new();
    t["provider"] = toml_edit::value(cfg.provider.as_str());
    t["model"] = toml_edit::value(cfg.model.as_str());
    t["protocol"] = toml_edit::value(cfg.protocol.as_str());
    if let Some(d) = cfg.dimensions {
        t["dimensions"] = toml_edit::value(i64::from(d));
    }
    t["encoding"] = toml_edit::value(cfg.encoding.as_str());
    t["batch_size"] = toml_edit::value(i64::from(cfg.batch_size));
    t["max_input_tokens"] = toml_edit::value(i64::from(cfg.max_input_tokens));
    t
}

fn write_reranker_table(cfg: &RerankerModelConfig) -> toml_edit::Table {
    let mut t = toml_edit::Table::new();
    t["provider"] = toml_edit::value(cfg.provider.as_str());
    t["model"] = toml_edit::value(cfg.model.as_str());
    t["protocol"] = toml_edit::value(cfg.protocol.as_str());
    if let Some(ep) = &cfg.endpoint {
        t["endpoint"] = toml_edit::value(ep.as_str());
    }
    t["batch_size"] = toml_edit::value(i64::from(cfg.batch_size));
    t["max_input_tokens"] = toml_edit::value(i64::from(cfg.max_input_tokens));
    t
}

fn write_profile_table(cfg: &RetrievalProfileConfig) -> toml_edit::Table {
    let mut t = toml_edit::Table::new();
    t["embedding_models"] = string_array(&cfg.embedding_models);
    t["reranker_models"] = string_array(&cfg.reranker_models);
    t["fallback_strategy"] = toml_edit::value(cfg.fallback_strategy.as_str());
    t["max_candidates"] = toml_edit::value(i64::from(cfg.max_candidates));
    t["max_results"] = toml_edit::value(i64::from(cfg.max_results));
    t["min_score"] = toml_edit::value(f64::from(cfg.min_score));
    t["deadline_ms"] = toml_edit::value(i64::try_from(cfg.deadline_ms).unwrap_or(i64::MAX));
    t["max_attempts"] = toml_edit::value(i64::from(cfg.max_attempts));
    t["max_input_tokens"] = toml_edit::value(i64::from(cfg.max_input_tokens));
    t["max_output_tokens"] = toml_edit::value(i64::from(cfg.max_output_tokens));
    t
}

fn string_array(items: &[String]) -> toml_edit::Item {
    let mut arr = toml_edit::Array::new();
    for s in items {
        arr.push(s.as_str());
    }
    toml_edit::Item::Value(toml_edit::Value::Array(arr))
}

fn write_prime(doc: &mut toml_edit::DocumentMut, prime: &PrimeConfig) {
    // Only write prime when something is non-default-ish (enabled or profile set),
    // but always rewrite if section already exists so saves are consistent.
    let has_content = prime.skills.enabled
        || prime.agents.enabled
        || prime.skills.retrieval_profile.is_some()
        || prime.agents.retrieval_profile.is_some()
        || doc.contains_key("prime");
    if !has_content {
        return;
    }
    let mut section = toml_edit::Table::new();
    section.set_implicit(true);
    section.insert(
        "skills",
        toml_edit::Item::Table(write_skill_table(&prime.skills)),
    );
    section.insert(
        "agents",
        toml_edit::Item::Table(write_agent_table(&prime.agents)),
    );
    // Preserve unknown top-level prime keys.
    if let Some(existing) = doc.get("prime").and_then(|i| i.as_table()) {
        for (k, v) in existing.iter() {
            if k != "skills" && k != "agents" && !section.contains_key(k) {
                section.insert(k, v.clone());
            }
        }
    }
    doc["prime"] = toml_edit::Item::Table(section);
}

fn write_skill_table(cfg: &xai_grok_config_types::SkillPrimeConfig) -> toml_edit::Table {
    let mut t = toml_edit::Table::new();
    t["enabled"] = toml_edit::value(cfg.enabled);
    if let Some(p) = &cfg.retrieval_profile {
        t["retrieval_profile"] = toml_edit::value(p.as_str());
    }
    t["max_results"] = toml_edit::value(i64::from(cfg.max_results));
    t["max_body_chars"] = toml_edit::value(i64::from(cfg.max_body_chars));
    t["max_total_chars"] = toml_edit::value(i64::from(cfg.max_total_chars));
    t["max_tokens"] = toml_edit::value(i64::from(cfg.max_tokens));
    t["max_context_fraction"] = toml_edit::value(f64::from(cfg.max_context_fraction));
    t["deadline_ms"] = toml_edit::value(i64::try_from(cfg.deadline_ms).unwrap_or(i64::MAX));
    t["degrade_on_error"] = toml_edit::value(cfg.degrade_on_error);
    t
}

fn write_agent_table(cfg: &xai_grok_config_types::AgentPrimeConfig) -> toml_edit::Table {
    let mut t = toml_edit::Table::new();
    t["enabled"] = toml_edit::value(cfg.enabled);
    if let Some(p) = &cfg.retrieval_profile {
        t["retrieval_profile"] = toml_edit::value(p.as_str());
    }
    t["max_results"] = toml_edit::value(i64::from(cfg.max_results));
    t["max_body_chars"] = toml_edit::value(i64::from(cfg.max_body_chars));
    t["max_total_chars"] = toml_edit::value(i64::from(cfg.max_total_chars));
    t["max_tokens"] = toml_edit::value(i64::from(cfg.max_tokens));
    t["max_context_fraction"] = toml_edit::value(f64::from(cfg.max_context_fraction));
    t["deadline_ms"] = toml_edit::value(i64::try_from(cfg.deadline_ms).unwrap_or(i64::MAX));
    t["degrade_on_error"] = toml_edit::value(cfg.degrade_on_error);
    t
}

fn write_memory_section(doc: &mut toml_edit::DocumentMut, graph: &RetrievalGraphConfig) {
    let has_any = graph.memory_retrieval_profile.is_some()
        || graph.memory_mode.is_some()
        || graph.memory_vector_store.is_some();
    if has_any && !doc.contains_key("memory") {
        doc["memory"] = toml_edit::Item::Table(toml_edit::Table::new());
    }
    if let Some(mem) = doc.get_mut("memory").and_then(|i| i.as_table_mut()) {
        if let Some(p) = graph.memory_retrieval_profile.as_deref().filter(|s| !s.is_empty()) {
            mem["retrieval_profile"] = toml_edit::value(p);
        } else {
            mem.remove("retrieval_profile");
        }
        if let Some(m) = graph.memory_mode {
            mem["mode"] = toml_edit::value(m.as_str());
        }
        if let Some(vs) = graph.memory_vector_store.as_deref().filter(|s| !s.is_empty()) {
            mem["vector_store"] = toml_edit::value(vs);
        } else {
            mem.remove("vector_store");
        }
    }
}

fn atomic_write_document(path: &Path, doc: &toml_edit::DocumentMut) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refusing to write config.toml through a symlink",
            ));
        }
        Ok(_) | Err(_) => {}
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
    {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&tmp)?;
        f.write_all(doc.to_string().as_bytes())?;
        f.flush()?;
        f.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(&tmp)?;
        let mut perms = meta.permissions();
        if perms.mode() & 0o777 != 0o600 {
            perms.set_mode(0o600);
            fs::set_permissions(&tmp, perms)?;
        }
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perms = meta.permissions();
            if perms.mode() & 0o777 != 0o600 {
                perms.set_mode(0o600);
                let _ = fs::set_permissions(path, perms);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use xai_grok_config_types::EmbeddingModelConfig;

    #[test]
    fn preserves_unrelated_sections_and_unknown_fields() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            r#"
# top comment
[models]
default = "grok"

[embedding_models.e1]
provider = "openai"
model = "m"
future_knob = 9

[other]
x = 1
"#,
        )
        .unwrap();
        let mut graph = RetrievalGraphConfig::default();
        graph.embedding_models.insert(
            "e1".into(),
            EmbeddingModelConfig {
                provider: "openai".into(),
                model: "m2".into(),
                ..Default::default()
            },
        );
        write_retrieval_graph(&path, &graph).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("default = \"grok\"")
                || text.contains("default = 'grok'")
                || text.contains("[models]")
        );
        assert!(text.contains("future_knob"));
        assert!(text.contains("[other]") || text.contains("x = 1"));
        assert!(text.contains("m2"));
    }

    #[test]
    fn clears_optional_dimensions_and_endpoint_when_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            r#"
[embedding_models.e1]
provider = "openai"
model = "m"
dimensions = 1536
future_knob = 9

[reranker_models.r1]
provider = "openai"
model = "rr"
endpoint = "v1/rerank"
"#,
        )
        .unwrap();
        let mut graph = RetrievalGraphConfig::default();
        graph.embedding_models.insert(
            "e1".into(),
            EmbeddingModelConfig {
                provider: "openai".into(),
                model: "m".into(),
                dimensions: None,
                ..Default::default()
            },
        );
        graph.reranker_models.insert(
            "r1".into(),
            xai_grok_config_types::RerankerModelConfig {
                provider: "openai".into(),
                model: "rr".into(),
                endpoint: None,
                ..Default::default()
            },
        );
        write_retrieval_graph(&path, &graph).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("dimensions"),
            "cleared dimensions must not resurrect: {text}"
        );
        assert!(
            !text.contains("endpoint"),
            "cleared endpoint must not resurrect: {text}"
        );
        assert!(
            text.contains("future_knob"),
            "unknown fields must remain: {text}"
        );
    }
}
