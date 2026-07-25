//! Shared baseline inventory types and loaders.

use super::domain::{
    ApiFamily, BindingStatus, ClaimSurface, CompatibilityStatus, Evidence, EvidenceKind,
    HttpMethod, OperationClaim, OperationIdentity, Transport, claim_is_consistent,
    media_type_is_valid, path_is_safe, sha256_hex_is_valid, source_revision_is_valid,
    timestamp_is_rfc3339_utc,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BaselineMeta {
    pub title: Option<String>,
    pub version: Option<String>,
    pub openapi: Option<String>,
    pub source_url: String,
    #[serde(default)]
    pub source_revision: Option<String>,
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
    #[serde(default)]
    pub converted_json_sha256: Option<String>,
    pub path_count: u64,
    pub endpoint_count: u64,
    pub schema_count: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct InventoryEndpoint {
    pub method: String,
    pub path: String,
    pub operation_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub summary: Option<String>,
    /// Prefer v2 field; accept legacy `content_types` as request types only.
    #[serde(default, alias = "content_types")]
    pub request_content_types: Vec<String>,
    #[serde(default)]
    pub response_content_types: Vec<String>,
    /// Multi-label transports. Legacy single `transport` string is upgraded.
    #[serde(default)]
    pub transports: Vec<String>,
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

    pub fn transport_set(&self) -> Result<Vec<Transport>, String> {
        let mut labels = self.transports.clone();
        if labels.is_empty()
            && let Some(t) = &self.transport
        {
            labels.push(t.clone());
        }
        if labels.is_empty() {
            return Err(format!(
                "endpoint {} missing transports",
                self.method_path_key()
            ));
        }
        let mut out = Vec::with_capacity(labels.len());
        let mut seen = HashSet::new();
        for raw in labels {
            let t = Transport::parse_strict(&raw).ok_or_else(|| {
                format!(
                    "invalid transport label `{raw}` on {}",
                    self.method_path_key()
                )
            })?;
            if seen.insert(t) {
                out.push(t);
            }
        }
        out.sort_by_key(|t| t.as_str());
        Ok(out)
    }

    pub fn to_identity(&self) -> Result<OperationIdentity, String> {
        let method = self
            .http_method()
            .ok_or_else(|| format!("invalid method {}", self.method))?;
        if !path_is_safe(&self.path) {
            return Err(format!("unsafe path {}", self.path));
        }
        for mt in self
            .request_content_types
            .iter()
            .chain(self.response_content_types.iter())
        {
            if !media_type_is_valid(mt) {
                return Err(format!(
                    "malformed media type `{mt}` on {}",
                    self.method_path_key()
                ));
            }
        }
        Ok(OperationIdentity {
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
            transports: self.transport_set()?,
            request_content_types: self.request_content_types.clone(),
            response_content_types: self.response_content_types.clone(),
        })
    }
}

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
        if self.format_version < 2 {
            return Err(format!(
                "inventory format_version {} < 2 (transport/content contract required)",
                self.format_version
            ));
        }
        if !sha256_hex_is_valid(&self.baseline.content_sha256) {
            return Err("content_sha256 must be 64 hex chars".into());
        }
        if let Some(conv) = &self.baseline.converted_json_sha256
            && !sha256_hex_is_valid(conv)
        {
            return Err("converted_json_sha256 must be 64 hex chars when present".into());
        }
        if self.baseline.content_bytes == 0 {
            return Err("content_bytes must be non-zero".into());
        }
        if self.baseline.source_url.is_empty() {
            return Err("missing source_url".into());
        }
        // Mutable branch pins are rejected for OpenAI (must be commit-addressed).
        if self.provider == "openai" {
            let rev = self.baseline.source_revision.as_deref().unwrap_or("");
            if !source_revision_is_valid(rev) {
                return Err("openai source_revision must be full 40-char hex git SHA".into());
            }
            if self.baseline.source_url.contains("/master/")
                || self.baseline.source_url.contains("/main/")
            {
                return Err("openai source_url must not use mutable branch path".into());
            }
            if !self.baseline.source_url.contains(rev) {
                return Err("openai source_url must embed source_revision".into());
            }
        } else if let Some(rev) = &self.baseline.source_revision
            && !source_revision_is_valid(rev)
        {
            return Err("source_revision must be 40 hex chars when present".into());
        }
        if !timestamp_is_rfc3339_utc(&self.baseline.fetched_at_utc) {
            return Err(format!(
                "invalid fetched_at_utc `{}`",
                self.baseline.fetched_at_utc
            ));
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
        let mut op_ids = HashSet::new();
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
            if let Some(id) = ep.operation_id.as_deref() {
                // operationId uniqueness is preferred; warn-as-error if duplicate non-empty.
                if !id.is_empty() && !op_ids.insert(id.to_owned()) {
                    return Err(format!("duplicate operation_id `{id}`"));
                }
            }
            // Every endpoint must have validated transports + media types.
            ep.to_identity()?;
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

    /// Baseline-presence claims for **every** endpoint in this inventory.
    ///
    /// Status is `Supported` (operation exists in the official baseline).
    /// Client/CLI bindings remain `NotImplemented` in Change 4.
    pub fn baseline_presence_claims(&self) -> Result<Vec<OperationClaim>, String> {
        let mut out = Vec::with_capacity(self.endpoints.len());
        let mut seen = HashSet::new();
        for ep in &self.endpoints {
            let key = ep.method_path_key();
            if !seen.insert(key.clone()) {
                return Err(format!("duplicate endpoint key in claim ledger: {key}"));
            }
            let identity = ep.to_identity()?;
            let claim = OperationClaim {
                identity,
                surface: ClaimSurface::OpenaiBaselinePresence,
                status: CompatibilityStatus::Supported,
                evidence: vec![Evidence {
                    kind: EvidenceKind::OfficialBaseline,
                    source: format!("{}:{}", self.provider, self.baseline.content_sha256),
                    timestamp_utc: self.baseline.fetched_at_utc.clone(),
                    baseline_version: self.baseline.version.clone(),
                    content_sha256: Some(self.baseline.content_sha256.clone()),
                }],
                client_binding: BindingStatus::NotImplemented,
                cli_binding: BindingStatus::NotImplemented,
            };
            claim_is_consistent(&claim)?;
            out.push(claim);
        }
        if out.len() as u64 != self.baseline.endpoint_count {
            return Err(format!(
                "baseline presence claim count {} != endpoint_count {}",
                out.len(),
                self.baseline.endpoint_count
            ));
        }
        Ok(out)
    }

    /// Client-completeness claims for **every** endpoint in this inventory.
    ///
    /// Changes 9–14: every baseline operation has a typed client method and CLI
    /// route (`Implemented` bindings, `Supported` completeness).
    pub fn client_completeness_claims(&self) -> Result<Vec<OperationClaim>, String> {
        let mut out = Vec::with_capacity(self.endpoints.len());
        let mut seen = HashSet::new();
        for ep in &self.endpoints {
            let key = ep.method_path_key();
            if !seen.insert(key.clone()) {
                return Err(format!("duplicate endpoint key in claim ledger: {key}"));
            }
            let identity = ep.to_identity()?;
            let claim = OperationClaim {
                identity,
                surface: ClaimSurface::OpenaiClientCompleteness,
                status: CompatibilityStatus::Supported,
                evidence: vec![Evidence {
                    kind: EvidenceKind::ClientBinding,
                    source: "openai_platform::generated::bindings".into(),
                    timestamp_utc: self.baseline.fetched_at_utc.clone(),
                    baseline_version: self.baseline.version.clone(),
                    content_sha256: Some(self.baseline.content_sha256.clone()),
                }],
                client_binding: BindingStatus::Implemented,
                cli_binding: BindingStatus::Implemented,
            };
            claim_is_consistent(&claim)?;
            out.push(claim);
        }
        if out.len() as u64 != self.baseline.endpoint_count {
            return Err(format!(
                "client completeness claim count {} != endpoint_count {}",
                out.len(),
                self.baseline.endpoint_count
            ));
        }
        Ok(out)
    }
}

const OPENAI_JSON: &str = include_str!("../../baselines/openai/endpoint_inventory.json");
const OPENROUTER_JSON: &str = include_str!("../../baselines/openrouter/endpoint_inventory.json");

pub fn openai_inventory() -> &'static ProviderInventory {
    static INV: OnceLock<ProviderInventory> = OnceLock::new();
    INV.get_or_init(|| {
        serde_json::from_str(OPENAI_JSON).expect("OpenAI endpoint inventory must parse")
    })
}

pub fn openrouter_inventory() -> &'static ProviderInventory {
    static INV: OnceLock<ProviderInventory> = OnceLock::new();
    INV.get_or_init(|| {
        serde_json::from_str(OPENROUTER_JSON).expect("OpenRouter endpoint inventory must parse")
    })
}

pub fn inventory_report_json(inv: &ProviderInventory) -> serde_json::Value {
    serde_json::json!({
        "provider": inv.provider,
        "format_version": inv.format_version,
        "baseline": {
            "version": inv.baseline.version,
            "openapi": inv.baseline.openapi,
            "source_url": inv.baseline.source_url,
            "source_revision": inv.baseline.source_revision,
            "fetched_at_utc": inv.baseline.fetched_at_utc,
            "content_sha256": inv.baseline.content_sha256,
            "content_bytes": inv.baseline.content_bytes,
            "converted_json_sha256": inv.baseline.converted_json_sha256,
            "path_count": inv.baseline.path_count,
            "endpoint_count": inv.baseline.endpoint_count,
            "schema_count": inv.baseline.schema_count,
        },
        "endpoint_keys": inv.endpoints.iter().map(|e| e.method_path_key()).collect::<Vec<_>>(),
        "priority_endpoints": inv.coding_agent_priority_endpoints,
        // Defaults for unmapped endpoints only; see openai_platform bindings for
        // Implemented coverage of every baseline operation.
        "client_binding_default": BindingStatus::Implemented,
        "cli_binding_default": BindingStatus::Implemented,
        "note": "Change 9–14 bind every baseline op; use assert_zero_uncovered_operations",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_inventory_parses_and_validates() {
        let inv = openai_inventory();
        assert_eq!(inv.provider, "openai");
        assert_eq!(inv.format_version, 2);
        assert_eq!(
            inv.baseline.content_sha256,
            "b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da"
        );
        assert_eq!(inv.baseline.content_bytes, 2_827_615);
        assert_eq!(inv.baseline.fetched_at_utc, "2026-07-25T16:25:32Z");
        assert_eq!(
            inv.baseline.source_revision.as_deref(),
            Some("5c044be3bf3a42854e99e34616564eeb2124a317")
        );
        assert!(
            inv.baseline
                .source_url
                .contains("5c044be3bf3a42854e99e34616564eeb2124a317")
        );
        assert!(!inv.baseline.source_url.contains("/master/"));
        inv.validate_integrity().expect("openai integrity");
    }

    #[test]
    fn openrouter_inventory_parses_and_validates() {
        let inv = openrouter_inventory();
        assert_eq!(inv.provider, "openrouter");
        assert_eq!(inv.format_version, 2);
        assert_eq!(
            inv.baseline.content_sha256,
            "90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203"
        );
        assert_eq!(inv.baseline.fetched_at_utc, "2026-07-25T16:25:35Z");
        inv.validate_integrity().expect("openrouter integrity");
    }

    #[test]
    fn every_endpoint_has_validated_transports_and_media_types() {
        for inv in [openai_inventory(), openrouter_inventory()] {
            for ep in &inv.endpoints {
                let id = ep
                    .to_identity()
                    .unwrap_or_else(|_| panic!("{}", ep.method_path_key()));
                assert!(!id.transports.is_empty());
            }
        }
    }

    #[test]
    fn stream_flag_ops_include_json_and_sse() {
        let oa = openai_inventory();
        let chat = oa
            .endpoints
            .iter()
            .find(|e| e.method_path_key() == "POST /chat/completions")
            .unwrap();
        let t = chat.transport_set().unwrap();
        assert!(t.contains(&Transport::HttpJson));
        assert!(t.contains(&Transport::HttpSse));
    }

    #[test]
    fn openrouter_file_download_is_binary_not_sole_json() {
        let or = openrouter_inventory();
        let dl = or
            .endpoints
            .iter()
            .find(|e| e.method_path_key() == "GET /files/{file_id}/content")
            .unwrap();
        let t = dl.transport_set().unwrap();
        assert!(t.contains(&Transport::HttpBinary));
        assert_ne!(t.as_slice(), &[Transport::HttpJson]);
    }

    #[test]
    fn report_json_is_stable_shape() {
        let report = inventory_report_json(openai_inventory());
        assert_eq!(report["provider"], "openai");
        assert_eq!(report["client_binding_default"], "implemented");
        assert!(
            report["baseline"]["source_revision"]
                .as_str()
                .unwrap()
                .len()
                == 40
        );
    }

    #[test]
    fn invalid_transport_label_rejected() {
        let mut ep = openai_inventory().endpoints[0].clone();
        ep.transports = vec!["not_a_transport".into()];
        assert!(ep.transport_set().is_err());
    }

    #[test]
    fn malformed_media_type_rejected() {
        let mut ep = openai_inventory().endpoints[0].clone();
        ep.request_content_types = vec!["not-a-type".into()];
        assert!(ep.to_identity().is_err());
    }

    #[test]
    fn openai_generator_verifies_source_yaml_sha() {
        use std::process::Command;
        let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let generator = crate_dir.join("baselines/openai/generate_inventory.py");
        let mini = crate_dir.join("baselines/openai/fixtures/mini_openapi.json");
        // JSON-only mini path with wrong sha must fail.
        let status = Command::new("python3")
            .arg(&generator)
            .arg("--input")
            .arg(&mini)
            .arg("--output")
            .arg(std::env::temp_dir().join("oa-bad.json"))
            .arg("--fetched-at-utc")
            .arg("2026-07-25T16:25:32Z")
            .arg("--expect-source-sha256")
            .arg("00".repeat(32))
            .arg("--expect-source-bytes")
            .arg("999999")
            .status()
            .expect("spawn");
        assert!(!status.success(), "wrong sha must fail");
    }

    /// Optional full-blob regen when OPENAI_OPENAPI_YAML_PATH is set.
    #[test]
    fn openai_inventory_regenerates_from_local_yaml_when_path_set() {
        let Ok(path) = std::env::var("OPENAI_OPENAPI_YAML_PATH") else {
            return;
        };
        let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let generator = crate_dir.join("baselines/openai/generate_inventory.py");
        let inv_path = crate_dir.join("baselines/openai/endpoint_inventory.json");
        let status = std::process::Command::new("python3")
            .arg(&generator)
            .arg("--source-yaml")
            .arg(&path)
            .arg("--output")
            .arg(&inv_path)
            .arg("--fetched-at-utc")
            .arg("2026-07-25T16:25:32Z")
            .arg("--expect-source-sha256")
            .arg("b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da")
            .arg("--expect-source-bytes")
            .arg("2827615")
            .arg("--check")
            .status()
            .expect("spawn");
        assert!(
            status.success(),
            "checked-in inventory must match generator for OPENAI_OPENAPI_YAML_PATH"
        );
    }
}
