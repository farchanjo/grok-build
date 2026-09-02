//! Load and edit `config.toml` while preserving unrelated formatting and data.
//!
//! [`set_table_field`] writes `[<table>].<key>` (creating the table, file, and
//! parent directory when missing). [`remove_table_key`] deletes a key from that
//! table and is a no-op when the table or key is absent. [`set_hint`] is a thin
//! wrapper that writes into `[hints]`.
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

/// Replace `[hints].pinned_tools` with `tools`, creating the array only when
/// non-empty (an empty pin list removes the key to keep `config.toml` tidy).
pub(crate) fn set_pinned_tools(tools: &[String]) -> std::io::Result<()> {
    let path = xai_grok_tools::util::grok_home::grok_home().join("config.toml");
    set_pinned_tools_at(&path, tools)
}

/// Like [`set_pinned_tools`] but targeting an explicit `config.toml` path
/// (test seam).
fn set_pinned_tools_at(path: &Path, tools: &[String]) -> std::io::Result<()> {
    if tools.is_empty() {
        return remove_table_key_at(path, "hints", "pinned_tools");
    }
    let array = toml_edit::Array::from_iter(tools.iter().map(String::as_str));
    set_table_field_at(path, "hints", "pinned_tools", array)
}

/// Read `[hints].pinned_tools` from the on-disk `config.toml`. Deduplicated
/// (first-seen order), blank entries dropped. Missing key/file → empty;
/// malformed shapes degrade to empty (fail-open display only).
pub(crate) fn read_pinned_tools() -> Vec<String> {
    let path = xai_grok_tools::util::grok_home::grok_home().join("config.toml");
    read_pinned_tools_at(&path)
}

/// Like [`read_pinned_tools`] but reading an explicit path (test seam).
fn read_pinned_tools_at(path: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(doc) = content.parse::<toml_edit::DocumentMut>() else {
        return Vec::new();
    };
    let Some(items) = doc
        .get("hints")
        .and_then(|h| h.get("pinned_tools"))
        .and_then(|p| p.as_array())
    else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for item in items.iter() {
        let Some(name) = item.as_str() else {
            continue;
        };
        if name.is_empty() || !seen.insert(name.to_owned()) {
            continue;
        }
        out.push(name.to_owned());
    }
    out
}

/// Inclusive minimum ChatGPT subscription `context_window` override.
pub(crate) const CHATGPT_CONTEXT_WINDOW_MIN: u64 = 8_000;
/// Inclusive maximum ChatGPT subscription `context_window` override.
pub(crate) const CHATGPT_CONTEXT_WINDOW_MAX: u64 = 1_050_000;
/// Values above this may hit OpenAI long-context limits or pricing.
pub(crate) const CHATGPT_LONG_CONTEXT_THRESHOLD: u64 = 272_000;
/// Inclusive minimum ChatGPT subscription `auto_compact_threshold_percent`.
pub(crate) const CHATGPT_AUTO_COMPACT_THRESHOLD_MIN: u8 = 0;
/// Inclusive maximum ChatGPT subscription `auto_compact_threshold_percent`.
pub(crate) const CHATGPT_AUTO_COMPACT_THRESHOLD_MAX: u8 = 100;

/// Returns true when `model_id` is a safe `chatgpt-*` catalog id.
pub(crate) fn is_chatgpt_model_id(model_id: &str) -> bool {
    match model_id.strip_prefix("chatgpt-") {
        Some(rest)
            if !rest.is_empty()
                && model_id.len() <= 128
                && model_id.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_')
                }) =>
        {
            true
        }
        _ => false,
    }
}

/// TOML table path for a ChatGPT model override, such as
/// `model."chatgpt-gpt-5.6-sol"`.
pub(crate) fn chatgpt_model_table(model_id: &str) -> Option<String> {
    is_chatgpt_model_id(model_id).then(|| format!("model.{model_id:?}"))
}

pub(crate) fn chatgpt_context_window_in_range(tokens: u64) -> bool {
    (CHATGPT_CONTEXT_WINDOW_MIN..=CHATGPT_CONTEXT_WINDOW_MAX).contains(&tokens)
}

/// Safe model id for per-model request overrides. Catalog ids may carry an
/// instance prefix and upstream slashes (`zdr:z-ai/glm-5.3-flash`).
fn is_overridable_model_id(model_id: &str) -> bool {
    !model_id.is_empty()
        && model_id.len() <= 160
        && model_id.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b':' | b'/')
        })
}

/// TOML table path for a per-model override, such as `model."dr:z-ai/glm-5.3-flash"`.
fn model_param_table(model_id: &str) -> Option<String> {
    is_overridable_model_id(model_id).then(|| format!("model.{model_id:?}"))
}

/// A per-model API request override persisted in `[model."<id>"]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelParam {
    Temperature,
    TopP,
    MaxCompletionTokens,
}

impl ModelParam {
    pub(crate) fn key(self) -> &'static str {
        match self {
            ModelParam::Temperature => "temperature",
            ModelParam::TopP => "top_p",
            ModelParam::MaxCompletionTokens => "max_completion_tokens",
        }
    }
}

/// Persist or clear `[model."<id>"].<param>` for any catalog model.
///
/// `None` removes the key (reverting to the model default). Performs blocking
/// I/O under the same surgical `toml_edit` discipline as the ChatGPT
/// overrides: unrelated tables and formatting are preserved, and an
/// unparseable file is refused without clobbering.
pub(crate) fn write_model_param_f64(
    model_id: &str,
    param: ModelParam,
    value: Option<f64>,
) -> Result<(), String> {
    let table = model_param_table(model_id).ok_or_else(|| "invalid model id".to_owned())?;
    match value {
        Some(value) => {
            if !value.is_finite() {
                return Err("value must be finite".to_owned());
            }
            set_table_field(&table, param.key(), value).map_err(|error| error.to_string())
        }
        None => remove_table_key(&table, param.key()).map_err(|error| error.to_string()),
    }
}

/// Integer variant of [`write_model_param_f64`] for token counts.
pub(crate) fn write_model_param_u64(
    model_id: &str,
    param: ModelParam,
    value: Option<u64>,
) -> Result<(), String> {
    let table = model_param_table(model_id).ok_or_else(|| "invalid model id".to_owned())?;
    match value {
        Some(value) => {
            let value = i64::try_from(value).map_err(|error| error.to_string())?;
            set_table_field(&table, param.key(), value).map_err(|error| error.to_string())
        }
        None => remove_table_key(&table, param.key()).map_err(|error| error.to_string()),
    }
}

/// Read a per-model override for display. Read-only; `None` when the file,
/// table, or key is absent or the value is the wrong type.
pub(crate) fn read_model_param_f64(model_id: &str, param: ModelParam) -> Option<f64> {
    let path = xai_grok_tools::util::grok_home::grok_home().join("config.toml");
    let raw = std::fs::read_to_string(path).ok()?;
    let value: toml::Value = toml::from_str(&raw).ok()?;
    let table = value.get("model")?.get(model_id)?;
    table.get(param.key())?.as_float()
}

/// Integer variant of [`read_model_param_f64`].
pub(crate) fn read_model_param_u64(model_id: &str, param: ModelParam) -> Option<u64> {
    let path = xai_grok_tools::util::grok_home::grok_home().join("config.toml");
    let raw = std::fs::read_to_string(path).ok()?;
    let value: toml::Value = toml::from_str(&raw).ok()?;
    let table = value.get("model")?.get(model_id)?;
    table
        .get(param.key())?
        .as_integer()
        .and_then(|v| u64::try_from(v).ok())
}

pub(crate) fn chatgpt_auto_compact_threshold_in_range(percent: u8) -> bool {
    (CHATGPT_AUTO_COMPACT_THRESHOLD_MIN..=CHATGPT_AUTO_COMPACT_THRESHOLD_MAX).contains(&percent)
}

/// Persist or clear `[model."<chatgpt-*>"].context_window` in `config.toml`.
pub(crate) fn write_chatgpt_context_window(
    model_id: &str,
    tokens: Option<u64>,
) -> Result<(), String> {
    write_chatgpt_context_window_with(
        model_id,
        tokens,
        |table, value| set_table_field(table, "context_window", value),
        |table| remove_table_key(table, "context_window"),
    )
}

#[cfg(test)]
fn write_chatgpt_context_window_at(
    path: &Path,
    model_id: &str,
    tokens: Option<u64>,
) -> Result<(), String> {
    write_chatgpt_context_window_with(
        model_id,
        tokens,
        |table, value| set_table_field_at(path, table, "context_window", value),
        |table| remove_table_key_at(path, table, "context_window"),
    )
}

fn write_chatgpt_context_window_with(
    model_id: &str,
    tokens: Option<u64>,
    set_field: impl FnOnce(&str, i64) -> std::io::Result<()>,
    remove_key: impl FnOnce(&str) -> std::io::Result<()>,
) -> Result<(), String> {
    let table =
        chatgpt_model_table(model_id).ok_or_else(|| "invalid ChatGPT model id".to_owned())?;
    match tokens {
        Some(tokens) if chatgpt_context_window_in_range(tokens) => {
            let value = i64::try_from(tokens).map_err(|error| error.to_string())?;
            set_field(&table, value).map_err(|error| error.to_string())
        }
        Some(_) => Err("context window must be between 8,000 and 1,050,000 tokens".to_owned()),
        None => remove_key(&table).map_err(|error| error.to_string()),
    }
}

/// Persist or clear `[model."<chatgpt-*>"].auto_compact_threshold_percent` in
/// `config.toml`. `None` removes the key; values are clamped to 0..=100.
pub(crate) fn write_chatgpt_auto_compact_threshold(
    model_id: &str,
    percent: Option<u8>,
) -> Result<(), String> {
    write_chatgpt_auto_compact_threshold_with(
        model_id,
        percent,
        |table, value| set_table_field(table, "auto_compact_threshold_percent", value),
        |table| remove_table_key(table, "auto_compact_threshold_percent"),
    )
}

#[cfg(test)]
fn write_chatgpt_auto_compact_threshold_at(
    path: &Path,
    model_id: &str,
    percent: Option<u8>,
) -> Result<(), String> {
    write_chatgpt_auto_compact_threshold_with(
        model_id,
        percent,
        |table, value| set_table_field_at(path, table, "auto_compact_threshold_percent", value),
        |table| remove_table_key_at(path, table, "auto_compact_threshold_percent"),
    )
}

fn write_chatgpt_auto_compact_threshold_with(
    model_id: &str,
    percent: Option<u8>,
    set_field: impl FnOnce(&str, i64) -> std::io::Result<()>,
    remove_key: impl FnOnce(&str) -> std::io::Result<()>,
) -> Result<(), String> {
    let table =
        chatgpt_model_table(model_id).ok_or_else(|| "invalid ChatGPT model id".to_owned())?;
    match percent {
        Some(percent) if chatgpt_auto_compact_threshold_in_range(percent) => {
            set_field(&table, i64::from(percent)).map_err(|error| error.to_string())
        }
        Some(_) => Err("auto compact threshold must be between 0 and 100 percent".to_owned()),
        None => remove_key(&table).map_err(|error| error.to_string()),
    }
}

/// Read persisted per-model ChatGPT auto-compact thresholds from
/// `~/.grok/config.toml`. Returns an empty map when the file is missing or
/// unparseable; never writes. Non-`chatgpt-*` ids and values outside 0..=100
/// are skipped. Performs blocking I/O.
pub(crate) fn read_chatgpt_auto_compact_thresholds() -> std::collections::BTreeMap<String, u8> {
    let path = xai_grok_tools::util::grok_home::grok_home().join("config.toml");
    read_chatgpt_auto_compact_thresholds_at(&path)
}

fn read_chatgpt_auto_compact_thresholds_at(path: &Path) -> std::collections::BTreeMap<String, u8> {
    let Some(doc) = read_config_document_for_edit(path) else {
        return std::collections::BTreeMap::new();
    };
    let Some(model_table) = doc.get("model").and_then(toml_edit::Item::as_table) else {
        return std::collections::BTreeMap::new();
    };
    let mut thresholds = std::collections::BTreeMap::new();
    for (model_id, entry) in model_table.iter() {
        let Some(entry_table) = entry.as_table() else {
            continue;
        };
        if !is_chatgpt_model_id(model_id) {
            continue;
        }
        let Some(value) = entry_table
            .get("auto_compact_threshold_percent")
            .and_then(toml_edit::Item::as_value)
            .and_then(toml_edit::Value::as_integer)
        else {
            continue;
        };
        if !chatgpt_auto_compact_threshold_in_range_u8(value) {
            continue;
        }
        if let Ok(percent) = u8::try_from(value) {
            thresholds.insert(model_id.to_owned(), percent);
        }
    }
    thresholds
}

/// Range check for the i64 that TOML integers arrive as (0..=100 inclusive).
fn chatgpt_auto_compact_threshold_in_range_u8(value: i64) -> bool {
    let min = i64::from(CHATGPT_AUTO_COMPACT_THRESHOLD_MIN);
    let max = i64::from(CHATGPT_AUTO_COMPACT_THRESHOLD_MAX);
    (min..=max).contains(&value)
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
    table_for_path_mut(&mut doc, &table_path(table)?)?[key] = toml_edit::value(value);
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
    for (index, segment) in path.iter().enumerate() {
        let is_leaf = index + 1 == path.len();
        current = current
            .entry(segment)
            .or_insert_with(|| {
                let mut table = toml_edit::Table::new();
                // Keep intermediate parents implicit so a nested write such as
                // `[model."chatgpt-gpt-5.6-sol"]` does not emit an empty `[model]`.
                if !is_leaf {
                    table.set_implicit(true);
                }
                toml_edit::Item::Table(table)
            })
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
    fn set_pinned_tools_round_trips_and_preserves_siblings() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[ui]\ncompact_mode = false\n").unwrap();

        set_pinned_tools_at(&path, &["server__search".to_string(), "grep".to_string()]).unwrap();

        assert_eq!(
            read_pinned_tools_at(&path),
            vec!["server__search".to_string(), "grep".to_string()]
        );
        assert!(
            fs::read_to_string(&path).unwrap().contains("compact_mode"),
            "sibling [ui] should be preserved"
        );

        // Replacing the list overwrites in place.
        set_pinned_tools_at(&path, &["grep".to_string()]).unwrap();
        assert_eq!(read_pinned_tools_at(&path), vec!["grep".to_string()]);

        // Empty list removes the key entirely.
        set_pinned_tools_at(&path, &[]).unwrap();
        assert!(read_pinned_tools_at(&path).is_empty());
        let doc = read_config_document_for_edit(&path).expect("reparse");
        assert!(
            doc.get("hints")
                .and_then(|h| h.get("pinned_tools"))
                .is_none(),
            "empty pin list must remove the key"
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

        let created = fs::read_to_string(&path).unwrap();
        assert!(
            created.contains("[model.\"chatgpt-gpt-5.6-sol\"]"),
            "expected quoted model table header, got:\n{created}"
        );
        assert!(
            created.contains("context_window = 1000000"),
            "expected context_window = 1000000, got:\n{created}"
        );
        let created_doc = read_config_document_for_edit(&path).expect("reparse");
        assert_eq!(
            created_doc["model"]["chatgpt-gpt-5.6-sol"]["context_window"].as_integer(),
            Some(1_000_000),
        );

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
    fn remove_table_key_at_preserves_unrelated_tables() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[hints]\nproject_picker_disabled = true\n\n[model.\"chatgpt-gpt-5.6-sol\"]\ncontext_window = 1000000\n",
        )
        .unwrap();

        remove_table_key_at(&path, "model.\"chatgpt-gpt-5.6-sol\"", "context_window").unwrap();

        let doc = read_config_document_for_edit(&path).expect("reparse");
        assert_eq!(
            doc["hints"]["project_picker_disabled"].as_bool(),
            Some(true),
        );
        assert!(
            doc["model"]["chatgpt-gpt-5.6-sol"]
                .get("context_window")
                .is_none()
        );
    }

    #[test]
    fn remove_table_key_at_refuses_unparseable_file_without_clobbering_it() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let bad = "this is [not valid toml\n";
        fs::write(&path, bad).unwrap();

        let error = remove_table_key_at(&path, "model.\"chatgpt-gpt-5.6-sol\"", "context_window")
            .expect_err("malformed config must be rejected");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(fs::read_to_string(&path).unwrap(), bad);
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

    #[test]
    fn chatgpt_model_table_accepts_only_safe_chatgpt_ids() {
        assert_eq!(
            chatgpt_model_table("chatgpt-gpt-5.6-sol").as_deref(),
            Some("model.\"chatgpt-gpt-5.6-sol\"")
        );
        assert!(chatgpt_model_table("openai-gpt-5.6-sol").is_none());
        assert!(chatgpt_model_table("chatgpt-").is_none());
        assert!(chatgpt_model_table("chatgpt-gpt-5.6-sol\"]\nhacked=1").is_none());
        assert!(chatgpt_model_table("chatgpt-gpt 5").is_none());
    }

    #[test]
    fn write_chatgpt_context_window_at_sets_and_clears_override() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[hints]\nproject_picker_disabled = true\n").unwrap();

        write_chatgpt_context_window_at(&path, "chatgpt-gpt-5.6-sol", Some(1_000_000)).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("[model.\"chatgpt-gpt-5.6-sol\"]"));
        assert!(body.contains("context_window = 1000000"));
        assert!(body.contains("project_picker_disabled"));

        write_chatgpt_context_window_at(&path, "chatgpt-gpt-5.6-sol", None).unwrap();
        let doc = read_config_document_for_edit(&path).expect("reparse");
        assert!(
            doc["model"]["chatgpt-gpt-5.6-sol"]
                .get("context_window")
                .is_none()
        );
        assert_eq!(
            doc["hints"]["project_picker_disabled"].as_bool(),
            Some(true)
        );
    }

    #[test]
    fn write_chatgpt_context_window_at_rejects_invalid_id_and_range() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[hints]\nproject_picker_disabled = true\n").unwrap();

        let invalid_id =
            write_chatgpt_context_window_at(&path, "openai-gpt-5.6-sol", Some(100_000))
                .expect_err("non-chatgpt ids must be rejected");
        assert!(invalid_id.contains("invalid ChatGPT model id"));

        let too_small = write_chatgpt_context_window_at(&path, "chatgpt-gpt-5.6-sol", Some(7_999))
            .expect_err("below-min tokens must be rejected");
        assert!(too_small.contains("8,000"));

        let too_large =
            write_chatgpt_context_window_at(&path, "chatgpt-gpt-5.6-sol", Some(1_050_001))
                .expect_err("above-max tokens must be rejected");
        assert!(too_large.contains("1,050,000"));

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[hints]\nproject_picker_disabled = true\n"
        );
    }

    #[test]
    fn write_chatgpt_auto_compact_threshold_at_sets_and_clears_override() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[hints]\nproject_picker_disabled = true\n").unwrap();

        write_chatgpt_auto_compact_threshold_at(&path, "chatgpt-gpt-5.6-sol", Some(70)).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("[model.\"chatgpt-gpt-5.6-sol\"]"));
        assert!(body.contains("auto_compact_threshold_percent = 70"));
        assert!(body.contains("project_picker_disabled"));

        write_chatgpt_auto_compact_threshold_at(&path, "chatgpt-gpt-5.6-sol", Some(0)).unwrap();
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("auto_compact_threshold_percent = 0")
        );

        write_chatgpt_auto_compact_threshold_at(&path, "chatgpt-gpt-5.6-sol", None).unwrap();
        let doc = read_config_document_for_edit(&path).expect("reparse");
        assert!(
            doc["model"]["chatgpt-gpt-5.6-sol"]
                .get("auto_compact_threshold_percent")
                .is_none()
        );
        assert_eq!(
            doc["hints"]["project_picker_disabled"].as_bool(),
            Some(true)
        );
    }

    #[test]
    fn write_chatgpt_auto_compact_threshold_at_rejects_invalid_id_and_range() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[hints]\nproject_picker_disabled = true\n").unwrap();

        let invalid_id =
            write_chatgpt_auto_compact_threshold_at(&path, "openai-gpt-5.6-sol", Some(50))
                .expect_err("non-chatgpt ids must be rejected");
        assert!(invalid_id.contains("invalid ChatGPT model id"));

        let too_large =
            write_chatgpt_auto_compact_threshold_at(&path, "chatgpt-gpt-5.6-sol", Some(101))
                .expect_err("above-max percent must be rejected");
        assert!(too_large.contains("100 percent"));

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[hints]\nproject_picker_disabled = true\n"
        );

        // Both boundaries are accepted.
        write_chatgpt_auto_compact_threshold_at(&path, "chatgpt-gpt-5.6-sol", Some(0)).unwrap();
        write_chatgpt_auto_compact_threshold_at(&path, "chatgpt-gpt-5.6-sol", Some(100)).unwrap();
        let doc = read_config_document_for_edit(&path).expect("reparse");
        assert_eq!(
            doc["model"]["chatgpt-gpt-5.6-sol"]["auto_compact_threshold_percent"].as_integer(),
            Some(100),
        );
    }

    #[test]
    fn write_chatgpt_auto_compact_threshold_at_preserves_sibling_context_window() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        write_chatgpt_context_window_at(&path, "chatgpt-gpt-5.6-sol", Some(350_000)).unwrap();
        write_chatgpt_auto_compact_threshold_at(&path, "chatgpt-gpt-5.6-sol", Some(60)).unwrap();

        let doc = read_config_document_for_edit(&path).expect("reparse");
        assert_eq!(
            doc["model"]["chatgpt-gpt-5.6-sol"]["context_window"].as_integer(),
            Some(350_000),
        );
        assert_eq!(
            doc["model"]["chatgpt-gpt-5.6-sol"]["auto_compact_threshold_percent"].as_integer(),
            Some(60),
        );

        write_chatgpt_auto_compact_threshold_at(&path, "chatgpt-gpt-5.6-sol", None).unwrap();
        let doc = read_config_document_for_edit(&path).expect("reparse");
        assert!(
            doc["model"]["chatgpt-gpt-5.6-sol"]
                .get("auto_compact_threshold_percent")
                .is_none()
        );
        assert_eq!(
            doc["model"]["chatgpt-gpt-5.6-sol"]["context_window"].as_integer(),
            Some(350_000),
            "clearing the threshold must leave context_window intact"
        );
    }

    #[test]
    fn read_chatgpt_auto_compact_thresholds_at_round_trips_with_writer() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        write_chatgpt_auto_compact_threshold_at(&path, "chatgpt-gpt-5.6-sol", Some(85)).unwrap();
        let mut expected = std::collections::BTreeMap::new();
        expected.insert("chatgpt-gpt-5.6-sol".to_owned(), 85u8);
        assert_eq!(read_chatgpt_auto_compact_thresholds_at(&path), expected);

        write_chatgpt_auto_compact_threshold_at(&path, "chatgpt-gpt-5.6-sol", None).unwrap();
        assert!(
            read_chatgpt_auto_compact_thresholds_at(&path).is_empty(),
            "cleared threshold must read back as no override"
        );
    }

    #[test]
    fn read_chatgpt_auto_compact_thresholds_at_missing_or_unparseable_returns_empty() {
        let dir = tempdir().unwrap();
        assert!(
            read_chatgpt_auto_compact_thresholds_at(&dir.path().join("absent.toml")).is_empty(),
            "missing file must read as an empty map"
        );

        let path = dir.path().join("config.toml");
        let bad = "this is [not valid toml\n";
        fs::write(&path, bad).unwrap();
        assert!(
            read_chatgpt_auto_compact_thresholds_at(&path).is_empty(),
            "unparseable file must read as an empty map without writing"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), bad);
    }

    #[test]
    fn read_chatgpt_auto_compact_thresholds_at_skips_non_chatgpt_and_out_of_range() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[model.\"grok-4.5\"]\nauto_compact_threshold_percent = 70\n\n[model.\"chatgpt-gpt-5.6-sol\"]\nauto_compact_threshold_percent = 150\n\n[model.\"chatgpt-gpt-5.5\"]\nauto_compact_threshold_percent = 40\n",
        )
        .unwrap();

        let mut expected = std::collections::BTreeMap::new();
        expected.insert("chatgpt-gpt-5.5".to_owned(), 40u8);
        assert_eq!(
            read_chatgpt_auto_compact_thresholds_at(&path),
            expected,
            "non-chatgpt ids and out-of-range values must be skipped"
        );
    }
}
