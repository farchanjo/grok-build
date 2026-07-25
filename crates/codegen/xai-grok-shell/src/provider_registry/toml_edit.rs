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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("toml.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(doc.to_string().as_bytes())?;
        f.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, path)?;
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
}
