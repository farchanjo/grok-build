//! Pinned OpenRouter OpenAPI baseline inventory.
//!
//! Loads the compact endpoint/field inventory checked in under
//! `baselines/openrouter/`. Used as the deterministic contract surface for
//! OpenRouter recovery and later conformance work. Never performs network I/O.

use serde::Deserialize;
use std::sync::OnceLock;

/// Raw inventory JSON embedded at compile time.
const INVENTORY_JSON: &str = include_str!("../baselines/openrouter/endpoint_inventory.json");

/// Provenance metadata for the official OpenAPI document used to build the
/// inventory.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct OpenRouterBaselineMeta {
    pub title: Option<String>,
    pub version: Option<String>,
    pub openapi: Option<String>,
    pub source_url: String,
    pub docs_url: Option<String>,
    pub yaml_url: Option<String>,
    pub fetched_at_utc: String,
    pub content_sha256: String,
    pub content_bytes: u64,
    pub path_count: u64,
    pub endpoint_count: u64,
    pub schema_count: u64,
}

/// One OpenAPI path operation.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct OpenRouterEndpoint {
    pub method: String,
    pub path: String,
    pub operation_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub summary: Option<String>,
}

impl OpenRouterEndpoint {
    /// Stable method+path key used by inventory tests and conformance maps.
    pub fn key(&self) -> String {
        format!("{} {}", self.method.to_ascii_uppercase(), self.path)
    }
}

/// One schema property recorded in the field inventory.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OpenRouterSchemaField {
    pub name: String,
    pub required: bool,
    /// OpenAPI `type` may be a string or an array (nullable unions).
    #[serde(default)]
    pub r#type: Option<serde_json::Value>,
    #[serde(default)]
    pub r#ref: Option<String>,
    #[serde(default)]
    pub union: Option<bool>,
    /// Optional enum members (string or mixed JSON values).
    #[serde(default)]
    pub r#enum: Option<serde_json::Value>,
}

/// Field inventory for one OpenAPI component schema.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OpenRouterSchemaInventory {
    pub schema: String,
    pub field_count: usize,
    #[serde(default)]
    pub fields: Vec<OpenRouterSchemaField>,
}

/// Full compact inventory document.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OpenRouterEndpointInventory {
    pub format_version: u32,
    pub provider: String,
    pub baseline: OpenRouterBaselineMeta,
    pub endpoints: Vec<OpenRouterEndpoint>,
    #[serde(default)]
    pub coding_agent_schema_fields: Vec<OpenRouterSchemaInventory>,
    #[serde(default)]
    pub coding_agent_priority_endpoints: Vec<String>,
}

/// Parse and cache the checked-in inventory.
pub fn openrouter_endpoint_inventory() -> &'static OpenRouterEndpointInventory {
    static INVENTORY: OnceLock<OpenRouterEndpointInventory> = OnceLock::new();
    INVENTORY.get_or_init(|| {
        serde_json::from_str(INVENTORY_JSON)
            .expect("checked-in OpenRouter endpoint inventory must parse")
    })
}

/// Return whether the inventory lists the given `METHOD /path` key.
pub fn inventory_has_endpoint(method_path: &str) -> bool {
    openrouter_endpoint_inventory()
        .endpoints
        .iter()
        .any(|ep| ep.key() == method_path)
}

/// Owned field names for a named schema, when present in the inventory.
pub fn schema_field_names(schema: &str) -> Option<Vec<String>> {
    openrouter_endpoint_inventory()
        .coding_agent_schema_fields
        .iter()
        .find(|s| s.schema == schema)
        .map(|s| s.fields.iter().map(|f| f.name.clone()).collect())
}

/// Coding-agent priority endpoints from the inventory (source of truth).
pub fn coding_agent_priority_endpoints() -> &'static [String] {
    openrouter_endpoint_inventory()
        .coding_agent_priority_endpoints
        .as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::process::Command;

    #[test]
    fn inventory_parses_with_expected_provenance() {
        let inv = openrouter_endpoint_inventory();
        assert_eq!(inv.format_version, 2);
        assert_eq!(inv.provider, "openrouter");
        assert_eq!(
            inv.baseline.source_url,
            "https://openrouter.ai/openapi.json"
        );
        assert_eq!(
            inv.baseline.content_sha256,
            "90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203"
        );
        assert_eq!(inv.baseline.content_bytes, 1_653_634);
        assert_eq!(inv.baseline.fetched_at_utc, "2026-07-25T16:25:35Z");
        assert_eq!(inv.baseline.path_count, 69);
        assert_eq!(inv.baseline.endpoint_count, 89);
        assert_eq!(inv.baseline.schema_count, 712);
        assert_eq!(inv.endpoints.len() as u64, inv.baseline.endpoint_count);
    }

    #[test]
    fn inventory_integrity_counts_and_uniqueness() {
        let inv = openrouter_endpoint_inventory();

        // Distinct path count matches baseline.path_count.
        let paths: HashSet<&str> = inv.endpoints.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths.len() as u64, inv.baseline.path_count);

        // Endpoint keys unique; count matches baseline.
        let mut keys: Vec<String> = inv.endpoints.iter().map(OpenRouterEndpoint::key).collect();
        let before = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(before, keys.len());
        assert_eq!(before as u64, inv.baseline.endpoint_count);

        // Schema names unique; field_count matches fields.len().
        let mut schema_names = HashSet::new();
        for schema in &inv.coding_agent_schema_fields {
            assert!(
                schema_names.insert(schema.schema.as_str()),
                "duplicate schema {}",
                schema.schema
            );
            assert_eq!(
                schema.field_count,
                schema.fields.len(),
                "field_count mismatch for {}",
                schema.schema
            );
            let mut field_names = HashSet::new();
            for f in &schema.fields {
                assert!(
                    field_names.insert(f.name.as_str()),
                    "duplicate field {} in {}",
                    f.name,
                    schema.schema
                );
            }
        }

        // Required metadata present.
        assert!(!inv.baseline.content_sha256.is_empty());
        assert!(inv.baseline.content_bytes > 0);
        assert!(!inv.baseline.fetched_at_utc.is_empty());
        assert!(!inv.baseline.source_url.is_empty());
    }

    #[test]
    fn coding_agent_priority_endpoints_all_exist() {
        let inv = openrouter_endpoint_inventory();
        assert!(
            !inv.coding_agent_priority_endpoints.is_empty(),
            "priority list must be non-empty"
        );
        for key in &inv.coding_agent_priority_endpoints {
            assert!(
                inventory_has_endpoint(key),
                "priority endpoint missing from endpoints: {key}"
            );
        }
    }

    #[test]
    fn chat_request_inventory_includes_routing_and_tools() {
        let fields = schema_field_names("ChatRequest").expect("ChatRequest inventory");
        for required in [
            "messages",
            "model",
            "models",
            "provider",
            "plugins",
            "tools",
            "tool_choice",
            "stream",
            "response_format",
            "reasoning",
            "reasoning_effort",
        ] {
            assert!(
                fields.iter().any(|f| f == required),
                "ChatRequest missing field {required}; got {fields:?}"
            );
        }
    }

    #[test]
    fn responses_request_inventory_includes_stateless_controls() {
        let fields = schema_field_names("ResponsesRequest").expect("ResponsesRequest inventory");
        for required in [
            "input",
            "model",
            "models",
            "provider",
            "plugins",
            "store",
            "previous_response_id",
            "stream",
            "tools",
            "reasoning",
        ] {
            assert!(
                fields.iter().any(|f| f == required),
                "ResponsesRequest missing field {required}; got {fields:?}"
            );
        }
    }

    #[test]
    fn provider_preferences_inventory_matches_openrouter_routing() {
        let fields =
            schema_field_names("ProviderPreferences").expect("ProviderPreferences inventory");
        for required in [
            "order",
            "only",
            "ignore",
            "allow_fallbacks",
            "require_parameters",
            "data_collection",
            "zdr",
            "quantizations",
            "sort",
            "max_price",
            "enforce_distillable_text",
            "preferred_max_latency",
            "preferred_min_throughput",
        ] {
            assert!(
                fields.iter().any(|f| f == required),
                "ProviderPreferences missing field {required}; got {fields:?}"
            );
        }
    }

    #[test]
    fn generator_runs_on_mini_fixture_without_network() {
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let generator = crate_dir.join("baselines/openrouter/generate_inventory.py");
        let mini = crate_dir.join("baselines/openrouter/fixtures/mini_openapi.json");
        let out = tempfile_path("mini-inventory.json");
        let status = Command::new("python3")
            .arg(&generator)
            .arg("--input")
            .arg(&mini)
            .arg("--output")
            .arg(&out)
            .arg("--fetched-at-utc")
            .arg("2026-07-24T00:00:00Z")
            .status()
            .expect("spawn generator");
        assert!(status.success(), "generator failed on mini fixture");
        let inv: OpenRouterEndpointInventory =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(inv.provider, "openrouter");
        assert!(!inv.coding_agent_priority_endpoints.is_empty());
        for key in &inv.coding_agent_priority_endpoints {
            assert!(
                inv.endpoints.iter().any(|e| e.key() == *key),
                "mini fixture missing priority {key}"
            );
        }
        let _ = std::fs::remove_file(&out);
    }

    /// When `OPENROUTER_OPENAPI_PATH` points at a local full OpenAPI blob,
    /// regenerate and require a byte-identical inventory (provenance pin).
    #[test]
    fn openrouter_inventory_regenerates_from_local_openapi_when_path_set() {
        let Ok(path) = std::env::var("OPENROUTER_OPENAPI_PATH") else {
            // Default unit-test path: skip when the pin blob is not supplied.
            return;
        };
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let generator = crate_dir.join("baselines/openrouter/generate_inventory.py");
        let inv_path = crate_dir.join("baselines/openrouter/endpoint_inventory.json");
        let status = Command::new("python3")
            .arg(&generator)
            .arg("--input")
            .arg(&path)
            .arg("--output")
            .arg(&inv_path)
            .arg("--fetched-at-utc")
            .arg("2026-07-25T16:25:35Z")
            .arg("--expect-source-sha256")
            .arg("90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203")
            .arg("--expect-source-bytes")
            .arg("1653634")
            .arg("--check")
            .status()
            .expect("spawn generator --check");
        assert!(
            status.success(),
            "checked-in inventory must match generator for OPENROUTER_OPENAPI_PATH={path}"
        );
    }

    use std::path::PathBuf;

    fn tempfile_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("openrouter-inv-{}-{}", std::process::id(), name));
        p
    }
}
