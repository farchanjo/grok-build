//! Shared baseline inventory types and loaders.

use super::domain::{
    ApiFamily, BindingStatus, HttpMethod, OperationIdentity, Transport, path_is_safe,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::OnceLock;

/// Provenance metadata shared by OpenAI and OpenRouter inventories.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BaselineMeta {
    pub title: Option<String>,
    pub version: Option<String>,
    pub openapi: Option<String>,
    pub source_url: String,
    #[serde(default)]
    pub docs_url: Option<String>,
    #[serde(default)]
    pub yaml_url: Option<String>,
    #[serde(default)]
    pub repo_url: Option<String>,
    #[serde(default)]
    pub license_note: Option<String>,
    #[serde(default)]
    pub source_format: Option<String>,
    pub fetched_at_utc: String,
    pub content_sha256: String,
    pub content_bytes: u64,
    pub path_count: u64,
    pub endpoint_count: u64,
    pub schema_count: u64,
}

/// One path operation from a compact inventory.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct InventoryEndpoint {
    pub method: String,
    pub path: String,
    pub operation_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub content_types: Vec<String>,
    #[serde(default)]
    pub transport: Option<String>,
}

impl InventoryEndpoint {
    pub fn method_path_key(&self) -> String {
        format!("{} {}", self.method.to_ascii_uppercase(), self.path)
    }

    pub fn http_method(&self) -> Option<HttpMethod> {
        HttpMethod::parse(&self.method)
    }

    pub fn transport(&self) -> Transport {
        self.transport
            .as_deref()
            .map(Transport::parse)
            .unwrap_or(Transport::Unknown)
    }

    pub fn to_identity(&self) -> Option<OperationIdentity> {
        let method = self.http_method()?;
        if !path_is_safe(&self.path) {
            return None;
        }
        Some(OperationIdentity {
            family: ApiFamily::from_path(&self.path),
            operation_id: self.operation_id.clone().unwrap_or_else(|| {
                format!(
                    "anon.{}{}",
                    method.as_str().to_ascii_lowercase(),
                    self.path.replace('/', ".")
                )
            }),
            method,
            path: self.path.clone(),
            transport: self.transport(),
            content_types: self.content_types.clone(),
        })
    }
}

/// Schema field inventory (optional, coding-agent focused).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SchemaField {
    pub name: String,
    pub required: bool,
    #[serde(default)]
    pub r#type: Option<serde_json::Value>,
    #[serde(default)]
    pub r#ref: Option<String>,
    #[serde(default)]
    pub union: Option<bool>,
    #[serde(default)]
    pub r#enum: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SchemaInventory {
    pub schema: String,
    pub field_count: usize,
    #[serde(default)]
    pub fields: Vec<SchemaField>,
}

/// Compact provider baseline inventory document.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ProviderInventory {
    pub format_version: u32,
    pub provider: String,
    pub baseline: BaselineMeta,
    pub endpoints: Vec<InventoryEndpoint>,
    #[serde(default)]
    pub coding_agent_schema_fields: Vec<SchemaInventory>,
    #[serde(default)]
    pub coding_agent_priority_endpoints: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl ProviderInventory {
    pub fn endpoint_map(&self) -> BTreeMap<String, &InventoryEndpoint> {
        self.endpoints
            .iter()
            .map(|e| (e.method_path_key(), e))
            .collect()
    }

    pub fn validate_integrity(&self) -> Result<(), String> {
        if self.baseline.content_sha256.is_empty() {
            return Err("missing content_sha256".into());
        }
        if self.baseline.content_bytes == 0 {
            return Err("content_bytes must be non-zero".into());
        }
        if self.baseline.source_url.is_empty() {
            return Err("missing source_url".into());
        }
        if self.endpoints.len() as u64 != self.baseline.endpoint_count {
            return Err(format!(
                "endpoint_count mismatch: meta={} actual={}",
                self.baseline.endpoint_count,
                self.endpoints.len()
            ));
        }
        let paths: HashSet<&str> = self.endpoints.iter().map(|e| e.path.as_str()).collect();
        if paths.len() as u64 != self.baseline.path_count {
            return Err(format!(
                "path_count mismatch: meta={} distinct={}",
                self.baseline.path_count,
                paths.len()
            ));
        }
        let mut keys = BTreeSet::new();
        for ep in &self.endpoints {
            if !path_is_safe(&ep.path) {
                return Err(format!("unsafe path: {}", ep.path));
            }
            if HttpMethod::parse(&ep.method).is_none() {
                return Err(format!("invalid method: {}", ep.method));
            }
            let key = ep.method_path_key();
            if !keys.insert(key.clone()) {
                return Err(format!("duplicate operation identity: {key}"));
            }
        }
        for schema in &self.coding_agent_schema_fields {
            if schema.field_count != schema.fields.len() {
                return Err(format!("schema {} field_count mismatch", schema.schema));
            }
        }
        for key in &self.coding_agent_priority_endpoints {
            if !keys.contains(key) {
                return Err(format!("priority endpoint missing: {key}"));
            }
        }
        Ok(())
    }
}

const OPENAI_JSON: &str = include_str!("../../baselines/openai/endpoint_inventory.json");
const OPENROUTER_JSON: &str = include_str!("../../baselines/openrouter/endpoint_inventory.json");

/// Parse and cache the OpenAI baseline inventory.
pub fn openai_inventory() -> &'static ProviderInventory {
    static INV: OnceLock<ProviderInventory> = OnceLock::new();
    INV.get_or_init(|| {
        serde_json::from_str(OPENAI_JSON).expect("OpenAI endpoint inventory must parse")
    })
}

/// Parse and cache the OpenRouter baseline inventory (shared loader).
pub fn openrouter_inventory() -> &'static ProviderInventory {
    static INV: OnceLock<ProviderInventory> = OnceLock::new();
    INV.get_or_init(|| {
        serde_json::from_str(OPENROUTER_JSON).expect("OpenRouter endpoint inventory must parse")
    })
}

/// Stable JSON report of a provider inventory (sorted keys via serde_json Value).
pub fn inventory_report_json(inv: &ProviderInventory) -> serde_json::Value {
    serde_json::json!({
        "provider": inv.provider,
        "format_version": inv.format_version,
        "baseline": {
            "version": inv.baseline.version,
            "openapi": inv.baseline.openapi,
            "source_url": inv.baseline.source_url,
            "fetched_at_utc": inv.baseline.fetched_at_utc,
            "content_sha256": inv.baseline.content_sha256,
            "content_bytes": inv.baseline.content_bytes,
            "path_count": inv.baseline.path_count,
            "endpoint_count": inv.baseline.endpoint_count,
            "schema_count": inv.baseline.schema_count,
        },
        "endpoint_keys": inv.endpoints.iter().map(|e| e.method_path_key()).collect::<Vec<_>>(),
        "priority_endpoints": inv.coding_agent_priority_endpoints,
        "client_binding_default": BindingStatus::NotImplemented,
        "cli_binding_default": BindingStatus::NotImplemented,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_inventory_parses_and_validates() {
        let inv = openai_inventory();
        assert_eq!(inv.provider, "openai");
        assert_eq!(inv.format_version, 1);
        assert_eq!(
            inv.baseline.content_sha256,
            "b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da"
        );
        assert_eq!(inv.baseline.content_bytes, 2_827_615);
        assert_eq!(inv.baseline.fetched_at_utc, "2026-07-25T17:00:00Z");
        assert_eq!(inv.baseline.version.as_deref(), Some("2.3.0"));
        inv.validate_integrity().expect("openai integrity");
        assert_eq!(inv.endpoints.len() as u64, inv.baseline.endpoint_count);
    }

    #[test]
    fn openrouter_inventory_parses_via_shared_loader() {
        let inv = openrouter_inventory();
        assert_eq!(inv.provider, "openrouter");
        assert_eq!(
            inv.baseline.content_sha256,
            "90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203"
        );
        inv.validate_integrity().expect("openrouter integrity");
    }

    #[test]
    fn report_json_is_stable_shape() {
        let report = inventory_report_json(openai_inventory());
        assert_eq!(report["provider"], "openai");
        assert!(report["endpoint_keys"].as_array().unwrap().len() > 0);
        assert_eq!(report["client_binding_default"], "not_implemented");
    }
}
