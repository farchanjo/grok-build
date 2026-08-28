//! Comment-preserving atomic TOML helpers for `[model_providers.<id>]`.

use super::id::{ProviderId, is_reserved_configured_id};
use super::lifecycle::{ProviderLifecycleError, validate_extra_headers, validate_http_base_url};
use indexmap::IndexMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Patch applied to one provider table.
#[derive(Debug, Clone, Default)]
pub struct ProviderTomlPatch {
    pub display_name: Option<String>,
    pub kind: Option<String>,
    pub base_url: Option<String>,
    pub admin_base_url: Option<String>,
    pub enabled: Option<bool>,
    pub default_backend: Option<String>,
    pub auth_scheme: Option<String>,
    pub env_key: Option<String>,
    pub admin_env_key: Option<String>,
    pub catalog_enabled: Option<bool>,
    pub capability_mode: Option<String>,
    pub catalog_ttl_secs: Option<u64>,
    pub request_timeout_secs: Option<u64>,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub extra_headers: Option<IndexMap<String, String>>,
    /// When set, replaces the entire `[model_providers.<id>.capabilities]` table.
    pub capabilities: Option<IndexMap<String, bool>>,
    /// OpenRouter fallback model slugs (`openrouter_fallback_models = [...]`).
    pub openrouter_fallback_models: Option<Vec<String>>,
    /// Explicit OpenRouter request-pacing opt-in.
    pub openrouter_pacing: Option<bool>,
    /// Provider-wide request `max_tokens`.
    pub max_completion_tokens: Option<u32>,
    /// Optional additive sidecar `api_surface`.
    pub api_surface: Option<String>,
    /// Optional additive sidecar `credential_route`.
    pub credential_route: Option<String>,
}

fn read_document(path: &Path) -> Result<toml_edit::DocumentMut, ProviderLifecycleError> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(ProviderLifecycleError::Validation(format!(
                "read config: {e}"
            )));
        }
    };
    text.parse::<toml_edit::DocumentMut>()
        .map_err(|e| ProviderLifecycleError::Validation(format!("parse config: {e}")))
}

fn atomic_write_document(path: &Path, doc: &toml_edit::DocumentMut) -> io::Result<()> {
    // Refuse to write through a symlink final path (no-follow target safety).
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

fn ensure_model_providers<'a>(doc: &'a mut toml_edit::DocumentMut) -> &'a mut toml_edit::Table {
    if !doc.contains_key("model_providers") {
        doc["model_providers"] = toml_edit::Item::Table(toml_edit::Table::new());
    }
    doc["model_providers"]
        .as_table_mut()
        .expect("model_providers table")
}

fn apply_patch_to_table(table: &mut toml_edit::Table, patch: &ProviderTomlPatch) {
    if let Some(v) = &patch.display_name {
        table["display_name"] = toml_edit::value(v.as_str());
    }
    if let Some(v) = &patch.kind {
        table["kind"] = toml_edit::value(v.as_str());
    }
    if let Some(v) = &patch.base_url {
        table["base_url"] = toml_edit::value(v.as_str());
    }
    if let Some(v) = &patch.admin_base_url {
        table["admin_base_url"] = toml_edit::value(v.as_str());
    }
    if let Some(v) = patch.enabled {
        table["enabled"] = toml_edit::value(v);
    }
    if let Some(v) = &patch.default_backend {
        table["default_backend"] = toml_edit::value(v.as_str());
    }
    if let Some(v) = &patch.auth_scheme {
        table["auth_scheme"] = toml_edit::value(v.as_str());
    }
    if let Some(v) = &patch.env_key {
        table["env_key"] = toml_edit::value(v.as_str());
    }
    if let Some(v) = &patch.admin_env_key {
        table["admin_env_key"] = toml_edit::value(v.as_str());
    }
    if let Some(v) = patch.catalog_enabled {
        table["catalog_enabled"] = toml_edit::value(v);
    }
    if let Some(v) = &patch.capability_mode {
        table["capability_mode"] = toml_edit::value(v.as_str());
    }
    if let Some(v) = patch.catalog_ttl_secs {
        table["catalog_ttl_secs"] = toml_edit::value(v as i64);
    }
    if let Some(v) = patch.request_timeout_secs {
        table["request_timeout_secs"] = toml_edit::value(v as i64);
    }
    if let Some(v) = &patch.organization {
        table["organization"] = toml_edit::value(v.as_str());
    }
    if let Some(v) = &patch.project {
        table["project"] = toml_edit::value(v.as_str());
    }
    if let Some(headers) = &patch.extra_headers {
        let mut h = toml_edit::Table::new();
        for (k, val) in headers {
            h[k.as_str()] = toml_edit::value(val.as_str());
        }
        table["extra_headers"] = toml_edit::Item::Table(h);
    }
    if let Some(caps) = &patch.capabilities {
        let mut c = toml_edit::Table::new();
        for (k, val) in caps {
            c[k.as_str()] = toml_edit::value(*val);
        }
        table["capabilities"] = toml_edit::Item::Table(c);
    }
    if let Some(models) = &patch.openrouter_fallback_models {
        let mut arr = toml_edit::Array::new();
        for m in models {
            arr.push(m.as_str());
        }
        table["openrouter_fallback_models"] = toml_edit::Item::Value(toml_edit::Value::Array(arr));
    }
    if let Some(v) = patch.openrouter_pacing {
        table["openrouter_pacing"] = toml_edit::value(v);
    }
    if let Some(v) = patch.max_completion_tokens {
        table["max_completion_tokens"] = toml_edit::value(i64::from(v));
    }
    if let Some(v) = &patch.api_surface {
        table["api_surface"] = toml_edit::value(v.as_str());
    }
    if let Some(v) = &patch.credential_route {
        table["credential_route"] = toml_edit::value(v.as_str());
    }
}

fn validate_patch(patch: &ProviderTomlPatch) -> Result<(), ProviderLifecycleError> {
    if let Some(url) = &patch.base_url {
        validate_http_base_url(url)?;
    }
    if let Some(url) = &patch.admin_base_url {
        validate_http_base_url(url)?;
    }
    if let Some(headers) = &patch.extra_headers {
        validate_extra_headers(headers)?;
    }
    if let Some(kind) = &patch.kind {
        let ok = matches!(
            kind.as_str(),
            "openai_compatible" | "custom" | "openai" | "openrouter" | "xai" | "zai"
        );
        if !ok {
            return Err(ProviderLifecycleError::Validation(format!(
                "unsupported provider kind `{kind}`"
            )));
        }
    }
    Ok(())
}

/// Insert or update a configured provider, preserving comments on other keys.
pub fn upsert_provider(
    config_path: &Path,
    provider_id: &ProviderId,
    patch: &ProviderTomlPatch,
    allow_reserved: bool,
) -> Result<(), ProviderLifecycleError> {
    if !allow_reserved && is_reserved_configured_id(provider_id.as_str()) {
        return Err(ProviderLifecycleError::ReservedId(
            provider_id.as_str().to_owned(),
        ));
    }
    validate_patch(patch)?;
    let mut doc = read_document(config_path)?;
    let providers = ensure_model_providers(&mut doc);
    if !providers.contains_key(provider_id.as_str()) {
        providers.insert(
            provider_id.as_str(),
            toml_edit::Item::Table(toml_edit::Table::new()),
        );
    }
    let table = providers
        .get_mut(provider_id.as_str())
        .and_then(|i| i.as_table_mut())
        .ok_or_else(|| {
            ProviderLifecycleError::Validation(format!(
                "model_providers.{} is not a table",
                provider_id.as_str()
            ))
        })?;
    // Default kind for new entries.
    if table.get("kind").is_none() {
        table["kind"] = toml_edit::value("openai_compatible");
    }
    apply_patch_to_table(table, patch);
    atomic_write_document(config_path, &doc)
        .map_err(|e| ProviderLifecycleError::Validation(e.to_string()))
}

pub fn apply_provider_patch(
    config_path: &Path,
    provider_id: &ProviderId,
    patch: &ProviderTomlPatch,
) -> Result<(), ProviderLifecycleError> {
    upsert_provider(config_path, provider_id, patch, true)
}

pub fn enable_provider(
    config_path: &Path,
    provider_id: &ProviderId,
) -> Result<(), ProviderLifecycleError> {
    apply_provider_patch(
        config_path,
        provider_id,
        &ProviderTomlPatch {
            enabled: Some(true),
            ..Default::default()
        },
    )
}

pub fn disable_provider(
    config_path: &Path,
    provider_id: &ProviderId,
) -> Result<(), ProviderLifecycleError> {
    apply_provider_patch(
        config_path,
        provider_id,
        &ProviderTomlPatch {
            enabled: Some(false),
            ..Default::default()
        },
    )
}

pub fn remove_provider(
    config_path: &Path,
    provider_id: &ProviderId,
) -> Result<(), ProviderLifecycleError> {
    let mut doc = read_document(config_path)?;
    if let Some(providers) = doc
        .get_mut("model_providers")
        .and_then(|i| i.as_table_mut())
    {
        if providers.remove(provider_id.as_str()).is_none() {
            return Err(ProviderLifecycleError::NotFound(
                provider_id.as_str().to_owned(),
            ));
        }
    } else {
        return Err(ProviderLifecycleError::NotFound(
            provider_id.as_str().to_owned(),
        ));
    }
    atomic_write_document(config_path, &doc)
        .map_err(|e| ProviderLifecycleError::Validation(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn upsert_preserves_unrelated_comments_and_keys() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            r#"# top comment
[model_providers.existing]
kind = "openai_compatible"
base_url = "http://127.0.0.1:9/v1"
# keep me
enabled = true

[other]
x = 1
"#,
        )
        .unwrap();
        let id = ProviderId::new("local_vllm").unwrap();
        upsert_provider(
            &path,
            &id,
            &ProviderTomlPatch {
                base_url: Some("http://127.0.0.1:8000/v1".into()),
                display_name: Some("Local vLLM".into()),
                enabled: Some(true),
                kind: Some("openai_compatible".into()),
                ..Default::default()
            },
            false,
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("# top comment"));
        assert!(text.contains("# keep me"));
        assert!(text.contains("[other]"));
        assert!(text.contains("local_vllm"));
        assert!(text.contains("Local vLLM"));
        assert!(text.contains("existing"));
    }

    #[test]
    fn rejects_reserved_id_for_new_configured() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let id = ProviderId::new("openai").unwrap();
        let err = upsert_provider(
            &path,
            &id,
            &ProviderTomlPatch {
                base_url: Some("https://api.openai.com/v1".into()),
                ..Default::default()
            },
            false,
        )
        .unwrap_err();
        assert!(matches!(err, ProviderLifecycleError::ReservedId(_)));
    }

    #[test]
    fn openrouter_preferences_preserve_sibling_keys() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            r#"
[model_providers.openrouter_work]
kind = "openrouter"
base_url = "https://openrouter.ai/api/v1"
# keep
enabled = true
"#,
        )
        .unwrap();
        let id = ProviderId::new("openrouter_work").unwrap();
        apply_openrouter_preferences(
            &path,
            &id,
            &OpenRouterPrefsPatch {
                data_collection: Some(Some("deny".into())),
                require_parameters: Some(Some(true)),
                allow_fallbacks: Some(Some(false)),
                zdr: Some(Some(true)),
                order: Some(vec!["openai".into(), "anthropic".into()]),
                only: None,
                ignore: Some(vec!["deepinfra".into()]),
                quantizations: Some(vec!["int8".into()]),
                sort: Some(Some("latency".into())),
            },
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("# keep"));
        assert!(text.contains("data_collection"));
        assert!(text.contains("deny"));
        assert!(text.contains("require_parameters"));
        assert!(text.contains("latency"));
    }

    #[test]
    fn combined_patch_and_openrouter_is_single_atomic_write() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            r#"
[model_providers.or_work]
kind = "openrouter"
base_url = "https://openrouter.ai/api/v1"
enabled = true
"#,
        )
        .unwrap();
        let id = ProviderId::new("or_work").unwrap();
        apply_provider_patch_with_openrouter(
            &path,
            &id,
            &ProviderTomlPatch {
                display_name: Some("OR Work".into()),
                ..Default::default()
            },
            Some(&OpenRouterPrefsPatch {
                data_collection: Some(Some("deny".into())),
                order: Some(vec!["openai".into()]),
                ..Default::default()
            }),
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("OR Work"));
        assert!(text.contains("data_collection"));
        assert!(text.contains("deny"));
        assert!(text.contains("order"));
    }
}

/// Nested OpenRouter `provider_preferences` field patch.
#[derive(Debug, Clone, Default)]
pub struct OpenRouterPrefsPatch {
    pub data_collection: Option<Option<String>>,
    pub require_parameters: Option<Option<bool>>,
    pub allow_fallbacks: Option<Option<bool>>,
    pub zdr: Option<Option<bool>>,
    pub order: Option<Vec<String>>,
    pub only: Option<Vec<String>>,
    pub ignore: Option<Vec<String>>,
    pub quantizations: Option<Vec<String>>,
    pub sort: Option<Option<String>>,
}

fn apply_openrouter_prefs_to_table(table: &mut toml_edit::Table, patch: &OpenRouterPrefsPatch) {
    if !table.contains_key("provider_preferences") {
        table["provider_preferences"] = toml_edit::Item::Table(toml_edit::Table::new());
    }
    let prefs = table
        .get_mut("provider_preferences")
        .and_then(|i| i.as_table_mut())
        .expect("provider_preferences table just ensured");
    if let Some(opt) = &patch.data_collection {
        match opt {
            Some(v) => prefs["data_collection"] = toml_edit::value(v.as_str()),
            None => {
                let _ = prefs.remove("data_collection");
            }
        }
    }
    if let Some(opt) = patch.require_parameters {
        match opt {
            Some(v) => prefs["require_parameters"] = toml_edit::value(v),
            None => {
                let _ = prefs.remove("require_parameters");
            }
        }
    }
    if let Some(opt) = patch.allow_fallbacks {
        match opt {
            Some(v) => prefs["allow_fallbacks"] = toml_edit::value(v),
            None => {
                let _ = prefs.remove("allow_fallbacks");
            }
        }
    }
    if let Some(opt) = patch.zdr {
        match opt {
            Some(v) => prefs["zdr"] = toml_edit::value(v),
            None => {
                let _ = prefs.remove("zdr");
            }
        }
    }
    if let Some(opt) = &patch.sort {
        match opt {
            Some(v) => prefs["sort"] = toml_edit::value(v.as_str()),
            None => {
                let _ = prefs.remove("sort");
            }
        }
    }
    if let Some(order) = &patch.order {
        let mut arr = toml_edit::Array::new();
        for o in order {
            arr.push(o.as_str());
        }
        prefs["order"] = toml_edit::Item::Value(toml_edit::Value::Array(arr));
    }
    if let Some(only) = &patch.only {
        let mut arr = toml_edit::Array::new();
        for o in only {
            arr.push(o.as_str());
        }
        prefs["only"] = toml_edit::Item::Value(toml_edit::Value::Array(arr));
    }
    if let Some(ignore) = &patch.ignore {
        let mut arr = toml_edit::Array::new();
        for o in ignore {
            arr.push(o.as_str());
        }
        prefs["ignore"] = toml_edit::Item::Value(toml_edit::Value::Array(arr));
    }
    if let Some(q) = &patch.quantizations {
        let mut arr = toml_edit::Array::new();
        for o in q {
            arr.push(o.as_str());
        }
        prefs["quantizations"] = toml_edit::Item::Value(toml_edit::Value::Array(arr));
    }
}

/// Nested OpenRouter `provider_preferences` patch applied under
/// `[model_providers.<id>.provider_preferences]`.
pub fn apply_openrouter_preferences(
    config_path: &Path,
    provider_id: &ProviderId,
    patch: &OpenRouterPrefsPatch,
) -> Result<(), ProviderLifecycleError> {
    apply_provider_patch_with_openrouter(
        config_path,
        provider_id,
        &ProviderTomlPatch::default(),
        Some(patch),
    )
}

/// Apply top-level provider fields and optional OpenRouter preferences in **one**
/// atomic document write (comment-preserving).
pub fn apply_provider_patch_with_openrouter(
    config_path: &Path,
    provider_id: &ProviderId,
    patch: &ProviderTomlPatch,
    openrouter: Option<&OpenRouterPrefsPatch>,
) -> Result<(), ProviderLifecycleError> {
    validate_patch(patch)?;
    let mut doc = read_document(config_path)?;
    let providers = ensure_model_providers(&mut doc);
    if !providers.contains_key(provider_id.as_str()) {
        return Err(ProviderLifecycleError::NotFound(
            provider_id.as_str().to_owned(),
        ));
    }
    let table = providers
        .get_mut(provider_id.as_str())
        .and_then(|i| i.as_table_mut())
        .ok_or_else(|| {
            ProviderLifecycleError::Validation(format!(
                "model_providers.{} is not a table",
                provider_id.as_str()
            ))
        })?;
    apply_patch_to_table(table, patch);
    if let Some(or_patch) = openrouter {
        apply_openrouter_prefs_to_table(table, or_patch);
    }
    atomic_write_document(config_path, &doc)
        .map_err(|e| ProviderLifecycleError::Validation(e.to_string()))
}
