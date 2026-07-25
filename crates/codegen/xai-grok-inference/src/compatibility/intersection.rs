//! Declared OpenAI ↔ OpenRouter intersection inventory.

use super::domain::{
    ApiFamily, BindingStatus, ClaimSurface, CompatibilityStatus, Evidence, EvidenceKind,
    HttpMethod, OperationClaim, OperationIdentity, Transport, path_is_safe,
};
use super::inventory::ProviderInventory;
use serde::Deserialize;
use std::collections::{BTreeSet, HashSet};
use std::sync::OnceLock;

const INTERSECTION_JSON: &str =
    include_str!("../../baselines/intersection/declared_intersection.json");

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct IntersectionBaselineRef {
    pub provider: String,
    pub version: Option<String>,
    pub content_sha256: String,
    pub fetched_at_utc: String,
    pub endpoint_count: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct IntersectionEvidence {
    pub kind: String,
    pub source: String,
    pub timestamp_utc: String,
    pub openai_baseline_version: Option<String>,
    pub openrouter_baseline_version: Option<String>,
    pub openai_content_sha256: String,
    pub openrouter_content_sha256: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct IntersectionMember {
    pub shared_id: String,
    pub api_family: String,
    pub method: String,
    pub path: String,
    pub openai_operation_id: String,
    pub openrouter_operation_id: String,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub content_types: Vec<String>,
    pub evidence: IntersectionEvidence,
    #[serde(default)]
    pub client_binding: String,
    #[serde(default)]
    pub cli_binding: String,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl IntersectionMember {
    pub fn method_path_key(&self) -> String {
        format!("{} {}", self.method.to_ascii_uppercase(), self.path)
    }

    pub fn to_identity(&self) -> Result<OperationIdentity, String> {
        if !path_is_safe(&self.path) {
            return Err(format!("unsafe path {}", self.path));
        }
        let method = HttpMethod::parse(&self.method)
            .ok_or_else(|| format!("invalid method {}", self.method))?;
        Ok(OperationIdentity {
            family: ApiFamily::from_path(&self.path),
            operation_id: self.shared_id.clone(),
            method,
            path: self.path.clone(),
            transport: self
                .transport
                .as_deref()
                .map(Transport::parse)
                .unwrap_or(Transport::Unknown),
            content_types: self.content_types.clone(),
        })
    }

    pub fn client_binding(&self) -> BindingStatus {
        BindingStatus::parse(&self.client_binding)
    }

    pub fn cli_binding(&self) -> BindingStatus {
        BindingStatus::parse(&self.cli_binding)
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OpenrouterNativeOp {
    pub method: String,
    pub path: String,
    pub operation_id: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub client_binding: String,
    #[serde(default)]
    pub cli_binding: String,
}

impl OpenrouterNativeOp {
    pub fn method_path_key(&self) -> String {
        format!("{} {}", self.method.to_ascii_uppercase(), self.path)
    }
}

/// Full declared intersection document.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DeclaredIntersection {
    pub format_version: u32,
    pub kind: String,
    pub generated_at_utc: String,
    pub openai_baseline: IntersectionBaselineRef,
    pub openrouter_baseline: IntersectionBaselineRef,
    pub members: Vec<IntersectionMember>,
    #[serde(default)]
    pub openrouter_native_operations: Vec<OpenrouterNativeOp>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl DeclaredIntersection {
    /// Every member must resolve in both baseline inventories; identities unique.
    pub fn validate_against_baselines(
        &self,
        openai: &ProviderInventory,
        openrouter: &ProviderInventory,
    ) -> Result<(), String> {
        if self.openai_baseline.content_sha256 != openai.baseline.content_sha256 {
            return Err("openai baseline sha mismatch vs inventory".into());
        }
        if self.openrouter_baseline.content_sha256 != openrouter.baseline.content_sha256 {
            return Err("openrouter baseline sha mismatch vs inventory".into());
        }
        if self.openai_baseline.endpoint_count != openai.baseline.endpoint_count {
            return Err("openai endpoint_count mismatch".into());
        }
        if self.openrouter_baseline.endpoint_count != openrouter.baseline.endpoint_count {
            return Err("openrouter endpoint_count mismatch".into());
        }

        let oa_map = openai.endpoint_map();
        let or_map = openrouter.endpoint_map();
        let mut shared_ids = HashSet::new();
        let mut keys = HashSet::new();

        for m in &self.members {
            if !shared_ids.insert(m.shared_id.clone()) {
                return Err(format!("duplicate shared_id {}", m.shared_id));
            }
            let key = m.method_path_key();
            if !keys.insert(key.clone()) {
                return Err(format!("duplicate method+path in intersection: {key}"));
            }
            let oa = oa_map
                .get(&key)
                .ok_or_else(|| format!("member {key} missing from OpenAI baseline"))?;
            let ore = or_map
                .get(&key)
                .ok_or_else(|| format!("member {key} missing from OpenRouter baseline"))?;
            // Operation ids must match the recorded baseline sides.
            if let Some(id) = oa.operation_id.as_deref()
                && id != m.openai_operation_id
            {
                return Err(format!(
                    "openai operation_id mismatch for {key}: inventory={id} declared={}",
                    m.openai_operation_id
                ));
            }
            if let Some(id) = ore.operation_id.as_deref()
                && id != m.openrouter_operation_id
            {
                return Err(format!(
                    "openrouter operation_id mismatch for {key}: inventory={id} declared={}",
                    m.openrouter_operation_id
                ));
            }
            // Bindings must not overclaim.
            if m.client_binding() == BindingStatus::Implemented
                || m.cli_binding() == BindingStatus::Implemented
            {
                return Err(format!(
                    "Change 4 forbids Implemented bindings for {}",
                    m.shared_id
                ));
            }
            m.to_identity()?;
        }

        // Native ops must be OpenRouter-only (not in OpenAI, present in OpenRouter).
        let mut native_keys = BTreeSet::new();
        for n in &self.openrouter_native_operations {
            let key = n.method_path_key();
            if !native_keys.insert(key.clone()) {
                return Err(format!("duplicate native op {key}"));
            }
            if oa_map.contains_key(&key) {
                return Err(format!(
                    "native op {key} also exists in OpenAI baseline (should be intersection)"
                ));
            }
            if !or_map.contains_key(&key) {
                return Err(format!("native op {key} missing from OpenRouter baseline"));
            }
            if BindingStatus::parse(&n.client_binding) == BindingStatus::Implemented
                || BindingStatus::parse(&n.cli_binding) == BindingStatus::Implemented
            {
                return Err(format!(
                    "native op {key} must not claim Implemented binding"
                ));
            }
        }
        Ok(())
    }

    /// Build auditable claims for intersection members (Supported on both baselines).
    pub fn openai_client_claims(&self) -> Vec<OperationClaim> {
        self.members
            .iter()
            .filter_map(|m| {
                let identity = m.to_identity().ok()?;
                Some(OperationClaim {
                    identity,
                    surface: ClaimSurface::OpenaiClientCompleteness,
                    // Presence in OpenAI baseline means the *operation exists*;
                    // typed client binding is still NotImplemented.
                    status: CompatibilityStatus::Supported,
                    evidence: vec![Evidence {
                        kind: EvidenceKind::InventoryDeclaration,
                        source: m.evidence.source.clone(),
                        timestamp_utc: m.evidence.timestamp_utc.clone(),
                        baseline_version: m.evidence.openai_baseline_version.clone(),
                        content_sha256: Some(m.evidence.openai_content_sha256.clone()),
                    }],
                    client_binding: m.client_binding(),
                    cli_binding: m.cli_binding(),
                })
            })
            .collect()
    }
}

/// Cached declared intersection.
pub fn declared_intersection() -> &'static DeclaredIntersection {
    static INV: OnceLock<DeclaredIntersection> = OnceLock::new();
    INV.get_or_init(|| {
        serde_json::from_str(INTERSECTION_JSON).expect("declared intersection must parse")
    })
}

/// Stable JSON report for CLI/docs consumers (later milestones).
pub fn intersection_report_json() -> serde_json::Value {
    let ix = declared_intersection();
    serde_json::json!({
        "kind": ix.kind,
        "format_version": ix.format_version,
        "generated_at_utc": ix.generated_at_utc,
        "member_count": ix.members.len(),
        "openrouter_native_count": ix.openrouter_native_operations.len(),
        "members": ix.members.iter().map(|m| serde_json::json!({
            "shared_id": m.shared_id,
            "method_path": m.method_path_key(),
            "openai_operation_id": m.openai_operation_id,
            "openrouter_operation_id": m.openrouter_operation_id,
            "api_family": m.api_family,
            "client_binding": m.client_binding,
            "cli_binding": m.cli_binding,
        })).collect::<Vec<_>>(),
        "openai_baseline_sha256": ix.openai_baseline.content_sha256,
        "openrouter_baseline_sha256": ix.openrouter_baseline.content_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compatibility::inventory::{openai_inventory, openrouter_inventory};

    #[test]
    fn intersection_parses_and_resolves_both_baselines() {
        let ix = declared_intersection();
        assert_eq!(ix.format_version, 1);
        assert_eq!(ix.kind, "openai_openrouter_declared_intersection");
        ix.validate_against_baselines(openai_inventory(), openrouter_inventory())
            .expect("intersection validates");
    }

    #[test]
    fn every_member_resolves_unambiguously() {
        let ix = declared_intersection();
        let oa = openai_inventory().endpoint_map();
        let or = openrouter_inventory().endpoint_map();
        assert!(!ix.members.is_empty());
        for m in &ix.members {
            let key = m.method_path_key();
            assert!(oa.contains_key(&key), "openai missing {key}");
            assert!(or.contains_key(&key), "openrouter missing {key}");
            assert_ne!(
                m.openai_operation_id, "",
                "openai operation id required for {}",
                m.shared_id
            );
            assert_ne!(
                m.openrouter_operation_id, "",
                "openrouter operation id required for {}",
                m.shared_id
            );
        }
    }

    #[test]
    fn bindings_are_not_implemented() {
        let ix = declared_intersection();
        for m in &ix.members {
            assert_eq!(m.client_binding(), BindingStatus::NotImplemented);
            assert_eq!(m.cli_binding(), BindingStatus::NotImplemented);
        }
        for n in &ix.openrouter_native_operations {
            assert_eq!(
                BindingStatus::parse(&n.client_binding),
                BindingStatus::NotImplemented
            );
            assert_eq!(
                BindingStatus::parse(&n.cli_binding),
                BindingStatus::NotImplemented
            );
        }
    }

    #[test]
    fn native_ops_are_openrouter_only() {
        let ix = declared_intersection();
        let oa = openai_inventory().endpoint_map();
        let or = openrouter_inventory().endpoint_map();
        assert!(!ix.openrouter_native_operations.is_empty());
        for n in &ix.openrouter_native_operations {
            let key = n.method_path_key();
            assert!(!oa.contains_key(&key), "native {key} leaked into OpenAI");
            assert!(or.contains_key(&key), "native {key} missing OpenRouter");
        }
    }

    #[test]
    fn report_json_serializes() {
        let report = intersection_report_json();
        assert!(report["member_count"].as_u64().unwrap() > 0);
        assert_eq!(report["members"][0]["client_binding"], "not_implemented");
    }

    #[test]
    fn claims_mark_supported_presence_with_not_implemented_bindings() {
        let claims = declared_intersection().openai_client_claims();
        assert!(!claims.is_empty());
        for c in claims {
            assert_eq!(c.status, CompatibilityStatus::Supported);
            assert_eq!(c.client_binding, BindingStatus::NotImplemented);
            assert_eq!(c.surface, ClaimSurface::OpenaiClientCompleteness);
        }
    }
}
