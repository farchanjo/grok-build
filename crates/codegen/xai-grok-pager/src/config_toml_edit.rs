//! Load and edit `config.toml` while preserving unrelated formatting and data.
//!
//! A non-empty file that does not parse is left untouched. Callers that write
//! table fields receive an error instead of overwriting malformed TOML.

use std::path::Path;

#[must_use]
pub(crate) fn read_config_document_for_edit(path: &Path) -> Option<toml_edit::DocumentMut> {
    #[allow(clippy::manual_unwrap_or_default)]
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => String::new(),
    };
    match content.parse() {
        Ok(d) => Some(d),
        Err(e) => {
            if content.is_empty() {
                return Some(toml_edit::DocumentMut::new());
            }
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "config.toml is not valid TOML; refusing to overwrite"
            );
            None
        }
    }
}

/// Set `[<table>].<key>` to `value` in `~/.grok/config.toml`, preserving
/// every other key and table. Creates the file and parent dir when missing.
/// Returns an error without writing when the existing file is non-empty but
/// unparseable. Performs blocking I/O.
pub(crate) fn set_table_field(
    table: &str,
    key: &str,
    value: impl Into<toml_edit::Value>,
) -> std::io::Result<()> {
    let path = xai_grok_tools::util::grok_home::grok_home().join("config.toml");
    set_table_field_at(&path, table, key, value)
}

/// Remove `<key>` from `[<table>]` in `~/.grok/config.toml`.
///
/// This is a no-op when the table or key is absent. It returns an error without
/// writing when the existing file is non-empty but unparseable. Performs
/// blocking I/O.
pub(crate) fn remove_table_key(table: &str, key: &str) -> std::io::Result<()> {
    let path = xai_grok_tools::util::grok_home::grok_home().join("config.toml");
    remove_table_key_at(&path, table, key)
}

/// Backward-compatible wrapper for setting `[hints].<key>`.
pub(crate) fn set_hint(key: &str, value: impl Into<toml_edit::Value>) -> std::io::Result<()> {
    set_table_field("hints", key, value)
}

fn set_table_field_at(
    path: &Path,
    table: &str,
    key: &str,
    value: impl Into<toml_edit::Value>,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let Some(mut doc) = read_config_document_for_edit(path) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "config.toml is not valid TOML; refusing to overwrite",
        ));
    };
    table_for_path_mut(&mut doc, &table_path(table)?)?.insert(key, toml_edit::value(value));
    std::fs::write(path, doc.to_string())
}

fn remove_table_key_at(path: &Path, table: &str, key: &str) -> std::io::Result<()> {
    let Some(mut doc) = read_config_document_for_edit(path) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "config.toml is not valid TOML; refusing to overwrite",
        ));
    };
    let table_path = table_path(table)?;
    let mut current = doc.as_table_mut();
    for segment in table_path {
        let Some(item) = current.get_mut(&segment) else {
            return Ok(());
        };
        let Some(next) = item.as_table_mut() else {
            return Ok(());
        };
        current = next;
    }
    if current.remove(key).is_some() {
        std::fs::write(path, doc.to_string())?;
    }
    Ok(())
}

fn table_path(table: &str) -> std::io::Result<Vec<String>> {
    let doc: toml_edit::DocumentMut = format!("[{table}]\n").parse().map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid TOML table name `{table}`: {error}"),
        )
    })?;
    let mut path = Vec::new();
    let mut current = doc.as_table();
    loop {
        let tables: Vec<_> = current
            .iter()
            .filter_map(|(key, item)| item.as_table().map(|child| (key, child)))
            .collect();
        match tables.as_slice() {
            [] => break,
            [(key, child)] => {
                path.push((*key).to_owned());
                current = child;
            }
            _ => unreachable!("a TOML table header has one path"),
        }
    }
    if path.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "TOML table name cannot be empty",
        ));
    }
    Ok(path)
}

fn table_for_path_mut<'a>(
    doc: &'a mut toml_edit::DocumentMut,
    path: &[String],
) -> std::io::Result<&'a mut toml_edit::Table> {
    let mut current = doc.as_table_mut();
    for segment in path {
        current = current
            .entry(segment)
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
            .as_table_mut()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("config.toml `{segment}` is not a table"),
                )
            })?;
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn merge_round_trip_preserves_sibling_tables() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[ui]\ncompact_mode = false\n\n[mcpServers]\nx = \"y\"\n",
        )
        .unwrap();

        let mut doc = read_config_document_for_edit(&path).expect("parse");
        doc["ui"]["show_timestamps"] = toml_edit::value(false);
        fs::write(&path, doc.to_string()).unwrap();

        let body = fs::read_to_string(&path).unwrap();
        assert!(
            body.contains("show_timestamps") && body.contains("mcpServers"),
            "expected merged TOML, got:\n{body}"
        );
    }

    #[test]
    fn nonempty_unparseable_returns_none_and_leaves_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let bad = "this is [not valid toml\n";
        fs::write(&path, bad).unwrap();

        assert!(read_config_document_for_edit(&path).is_none());
        assert_eq!(fs::read_to_string(&path).unwrap(), bad);
    }

    #[test]
    fn missing_file_is_editable_empty_doc() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("absent.toml");
        let doc = read_config_document_for_edit(&path).expect("editable");
        assert!(!doc.contains_key("ui"));
    }

    #[test]
    fn set_hint_at_round_trips_and_preserves_siblings() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[ui]\ncompact_mode = false\n").unwrap();

        set_table_field_at(&path, "hints", "project_picker_disabled", true).unwrap();

        let doc = read_config_document_for_edit(&path).expect("reparse");
        assert_eq!(
            doc.get("hints")
                .and_then(|h| h.get("project_picker_disabled"))
                .and_then(|v| v.as_bool()),
            Some(true),
        );
        assert!(
            fs::read_to_string(&path).unwrap().contains("compact_mode"),
            "sibling [ui] should be preserved"
        );
    }

    #[test]
    fn set_hint_at_creates_missing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested/config.toml");
        set_table_field_at(&path, "hints", "project_picker_disabled", true).unwrap();
        assert!(
            path.exists(),
            "missing file and parent dir should be created"
        );
    }

    #[test]
    fn set_hint_write_then_read_back_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[ui]\ntheme = \"dark\"\n").unwrap();

        set_table_field_at(&path, "hints", "project_picker_disabled", true).unwrap();

        let doc = read_config_document_for_edit(&path).expect("reparse");
        let disabled = doc
            .get("hints")
            .and_then(|h| h.get("project_picker_disabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(disabled, "should read back true after set_hint write");
    }

    #[test]
    fn set_table_field_at_refuses_unparseable_file_without_clobbering_it() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let bad = "this is [not valid toml\n";
        fs::write(&path, bad).unwrap();

        let error = set_table_field_at(&path, "hints", "project_picker_disabled", true)
            .expect_err("malformed config must be rejected");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(fs::read_to_string(&path).unwrap(), bad);
    }

    #[test]
    fn set_table_field_at_creates_and_updates_model_override() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        set_table_field_at(
            &path,
            "model.\"chatgpt-gpt-5.6-sol\"",
            "context_window",
            1_000_000,
        )
        .unwrap();
        set_table_field_at(
            &path,
            "model.\"chatgpt-gpt-5.6-sol\"",
            "context_window",
            272_000,
        )
        .unwrap();

        let doc = read_config_document_for_edit(&path).expect("reparse");
        assert_eq!(
            doc["model"]["chatgpt-gpt-5.6-sol"]["context_window"].as_integer(),
            Some(272_000),
        );
    }

    #[test]
    fn set_table_field_at_preserves_unrelated_tables() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[hints]\nproject_picker_disabled = true\n").unwrap();

        set_table_field_at(
            &path,
            "model.\"chatgpt-gpt-5.6-sol\"",
            "context_window",
            1_000_000,
        )
        .unwrap();

        let doc = read_config_document_for_edit(&path).expect("reparse");
        assert_eq!(
            doc["hints"]["project_picker_disabled"].as_bool(),
            Some(true),
        );
        assert_eq!(
            doc["model"]["chatgpt-gpt-5.6-sol"]["context_window"].as_integer(),
            Some(1_000_000),
        );
    }

    #[test]
    fn remove_table_key_at_is_idempotent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[model.\"chatgpt-gpt-5.6-sol\"]\ncontext_window = 1000000\n",
        )
        .unwrap();

        remove_table_key_at(&path, "model.\"chatgpt-gpt-5.6-sol\"", "context_window").unwrap();
        let after_first_remove = fs::read_to_string(&path).unwrap();
        remove_table_key_at(&path, "model.\"chatgpt-gpt-5.6-sol\"", "context_window").unwrap();

        let doc = read_config_document_for_edit(&path).expect("reparse");
        assert!(
            doc["model"]["chatgpt-gpt-5.6-sol"]
                .get("context_window")
                .is_none()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), after_first_remove);
    }

    #[test]
    fn vim_mode_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[ui]\ncompact_mode = false\n").unwrap();

        let mut doc = read_config_document_for_edit(&path).expect("parse");
        doc["ui"]["vim_mode"] = toml_edit::value(true);
        fs::write(&path, doc.to_string()).unwrap();

        let doc2 = read_config_document_for_edit(&path).expect("reparse");
        let enabled = doc2
            .get("ui")
            .and_then(|h| h.get("vim_mode"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(enabled, "expected vim_mode = true after round-trip");

        let body = fs::read_to_string(&path).unwrap();
        assert!(
            body.contains("compact_mode"),
            "sibling [ui] keys should be preserved"
        );
    }
}
