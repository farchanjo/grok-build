//! Declared OpenAI ↔ OpenRouter intersection inventory (semantic).

use super::domain::{
    ApiFamily, BindingStatus, ClaimSurface, CompatibilityStatus, Evidence, EvidenceKind,
    HttpMethod, OperationClaim, OperationIdentity, Transport, claim_is_consistent, path_is_safe,
};
use super::inventory::ProviderInventory;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::OnceLock;

const INTERSECTION_JSON: &str =
    include_str!("../../baselines/intersection/declared_intersection.json");

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct IntersectionBaselineRef {
    pub provider: String,
    pub version: Option<String>,
    pub content_sha256: String,
    #[serde(default)]
    pub source_revision: Option<String>,
    pub fetched_at_utc: String,
    pub endpoint_count: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SemanticEvidence {
    pub kind: String,
    #[serde(default)]
    pub openai_schema: Option<String>,
    #[serde(default)]
    pub openrouter_schema: Option<String>,
    pub rationale: String,
    #[serde(default)]
    pub openai_docs: Option<String>,
    #[serde(default)]
    pub openrouter_docs: Option<String>,
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
    #[serde(default)]
    pub openai_source_revision: Option<String>,
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
    pub transports: Vec<String>,
    #[serde(default)]
    pub request_content_types: Vec<String>,
    #[serde(default)]
    pub response_content_types: Vec<String>,
    pub semantic_evidence: SemanticEvidence,
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
        if self.semantic_evidence.rationale.trim().is_empty() {
            return Err(format!("missing semantic rationale for {}", self.shared_id));
        }
        let method = HttpMethod::parse(&self.method)
            .ok_or_else(|| format!("invalid method {}", self.method))?;
        let mut transports = Vec::new();
        for raw in &self.transports {
            let t = Transport::parse_strict(raw)
                .ok_or_else(|| format!("invalid transport `{raw}` on {}", self.shared_id))?;
            transports.push(t);
        }
        if transports.is_empty() {
            return Err(format!("empty transports for {}", self.shared_id));
        }
        Ok(OperationIdentity {
            family: ApiFamily::from_path(&self.path),
            operation_id: self.shared_id.clone(),
            method,
            path: self.path.clone(),
            transports,
            request_content_types: self.request_content_types.clone(),
            response_content_types: self.response_content_types.clone(),
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
pub struct CategorizedOpenrouterOp {
    pub method: String,
    pub path: String,
    pub operation_id: Option<String>,
    #[serde(default)]
    pub openai_operation_id: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub client_binding: String,
    #[serde(default)]
    pub cli_binding: String,
}

impl CategorizedOpenrouterOp {
    pub fn method_path_key(&self) -> String {
        format!("{} {}", self.method.to_ascii_uppercase(), self.path)
    }
}

/// Full declared intersection document (format v2).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DeclaredIntersection {
    pub format_version: u32,
    pub kind: String,
    pub generated_at_utc: String,
    pub openai_baseline: IntersectionBaselineRef,
    pub openrouter_baseline: IntersectionBaselineRef,
    pub members: Vec<IntersectionMember>,
    /// METHOD+path overlap without verified OpenAI-compatible semantics.
    #[serde(default)]
    pub same_path_unverified_overlap: Vec<CategorizedOpenrouterOp>,
    /// OpenRouter-only path operations (not in OpenAI baseline).
    #[serde(default)]
    pub openrouter_contract_outside_intersection: Vec<CategorizedOpenrouterOp>,
    /// Alias for path-exclusive OpenRouter ops (same as outside_intersection).
    #[serde(default)]
    pub openrouter_native_operations: Vec<CategorizedOpenrouterOp>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl DeclaredIntersection {
    pub fn validate_against_baselines(
        &self,
        openai: &ProviderInventory,
        openrouter: &ProviderInventory,
    ) -> Result<(), String> {
        if self.format_version < 2 {
            return Err("intersection format_version must be >= 2".into());
        }
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
        if let (Some(a), Some(b)) = (
            self.openai_baseline.source_revision.as_deref(),
            openai.baseline.source_revision.as_deref(),
        ) && a != b
        {
            return Err("openai source_revision mismatch".into());
        }

        let oa_map = openai.endpoint_map();
        let or_map = openrouter.endpoint_map();
        let mut shared_ids = HashSet::new();
        let mut ix_keys = HashSet::new();

        for m in &self.members {
            if m.semantic_evidence.rationale.trim().is_empty() {
                return Err(format!("member {} missing semantic rationale", m.shared_id));
            }
            if !shared_ids.insert(m.shared_id.clone()) {
                return Err(format!("duplicate shared_id {}", m.shared_id));
            }
            let key = m.method_path_key();
            if !ix_keys.insert(key.clone()) {
                return Err(format!("duplicate method+path in intersection: {key}"));
            }
            let oa = oa_map
                .get(&key)
                .ok_or_else(|| format!("member {key} missing from OpenAI baseline"))?;
            let ore = or_map
                .get(&key)
                .ok_or_else(|| format!("member {key} missing from OpenRouter baseline"))?;
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

        let mut unv_keys = HashSet::new();
        for n in &self.same_path_unverified_overlap {
            let key = n.method_path_key();
            if !unv_keys.insert(key.clone()) {
                return Err(format!("duplicate same_path_unverified {key}"));
            }
            if !oa_map.contains_key(&key) || !or_map.contains_key(&key) {
                return Err(format!(
                    "same_path_unverified {key} must exist in both baselines"
                ));
            }
            if ix_keys.contains(&key) {
                return Err(format!("{key} in both intersection and unverified overlap"));
            }
            if n.reason.as_deref().unwrap_or("").trim().is_empty() {
                return Err(format!("same_path_unverified {key} missing reason"));
            }
        }

        let mut nat_keys = HashSet::new();
        let outside = if self.openrouter_contract_outside_intersection.is_empty() {
            &self.openrouter_native_operations
        } else {
            &self.openrouter_contract_outside_intersection
        };
        for n in outside {
            let key = n.method_path_key();
            if !nat_keys.insert(key.clone()) {
                return Err(format!("duplicate outside-intersection op {key}"));
            }
            if oa_map.contains_key(&key) {
                return Err(format!(
                    "outside-intersection {key} also exists in OpenAI (use unverified overlap)"
                ));
            }
            if !or_map.contains_key(&key) {
                return Err(format!(
                    "outside-intersection {key} missing from OpenRouter baseline"
                ));
            }
        }

        // Disjoint + cover OpenRouter exactly.
        if !ix_keys.is_disjoint(&unv_keys)
            || !ix_keys.is_disjoint(&nat_keys)
            || !unv_keys.is_disjoint(&nat_keys)
        {
            return Err("OpenRouter category sets are not disjoint".into());
        }
        let covered: HashSet<String> = ix_keys
            .iter()
            .chain(unv_keys.iter())
            .chain(nat_keys.iter())
            .cloned()
            .collect();
        let or_keys: HashSet<String> = or_map.keys().cloned().collect();
        if covered != or_keys {
            let missing: Vec<_> = or_keys.difference(&covered).cloned().collect();
            let extra: Vec<_> = covered.difference(&or_keys).cloned().collect();
            return Err(format!(
                "OpenRouter partition incomplete: missing={missing:?} extra={extra:?}"
            ));
        }
        Ok(())
    }

    /// Baseline-presence claims only (not client completeness).
    pub fn openai_baseline_presence_claims(&self) -> Vec<OperationClaim> {
        self.members
            .iter()
            .filter_map(|m| {
                let identity = m.to_identity().ok()?;
                let claim = OperationClaim {
                    identity,
                    surface: ClaimSurface::OpenaiBaselinePresence,
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
                };
                claim_is_consistent(&claim).ok()?;
                Some(claim)
            })
            .collect()
    }

    /// Client-completeness claims are Unknown/NotImplemented in Change 4.
    pub fn openai_client_completeness_claims(&self) -> Vec<OperationClaim> {
        self.members
            .iter()
            .filter_map(|m| {
                let identity = m.to_identity().ok()?;
                let claim = OperationClaim {
                    identity,
                    surface: ClaimSurface::OpenaiClientCompleteness,
                    status: CompatibilityStatus::Unknown,
                    evidence: vec![Evidence {
                        kind: EvidenceKind::ClientBinding,
                        source: "change4_not_implemented".into(),
                        timestamp_utc: m.evidence.timestamp_utc.clone(),
                        baseline_version: m.evidence.openai_baseline_version.clone(),
                        content_sha256: Some(m.evidence.openai_content_sha256.clone()),
                    }],
                    client_binding: BindingStatus::NotImplemented,
                    cli_binding: BindingStatus::NotImplemented,
                };
                claim_is_consistent(&claim).ok()?;
                Some(claim)
            })
            .collect()
    }
}

pub fn declared_intersection() -> &'static DeclaredIntersection {
    static INV: OnceLock<DeclaredIntersection> = OnceLock::new();
    INV.get_or_init(|| {
        serde_json::from_str(INTERSECTION_JSON).expect("declared intersection must parse")
    })
}

pub fn intersection_report_json() -> serde_json::Value {
    let ix = declared_intersection();
    serde_json::json!({
        "kind": ix.kind,
        "format_version": ix.format_version,
        "generated_at_utc": ix.generated_at_utc,
        "member_count": ix.members.len(),
        "same_path_unverified_count": ix.same_path_unverified_overlap.len(),
        "openrouter_outside_count": ix.openrouter_contract_outside_intersection.len()
            .max(ix.openrouter_native_operations.len()),
        "members": ix.members.iter().map(|m| serde_json::json!({
            "shared_id": m.shared_id,
            "method_path": m.method_path_key(),
            "openai_operation_id": m.openai_operation_id,
            "openrouter_operation_id": m.openrouter_operation_id,
            "api_family": m.api_family,
            "semantic_rationale": m.semantic_evidence.rationale,
            "client_binding": m.client_binding,
            "cli_binding": m.cli_binding,
        })).collect::<Vec<_>>(),
        "openai_baseline_sha256": ix.openai_baseline.content_sha256,
        "openai_source_revision": ix.openai_baseline.source_revision,
        "openrouter_baseline_sha256": ix.openrouter_baseline.content_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compatibility::inventory::{openai_inventory, openrouter_inventory};

    #[test]
    fn intersection_parses_and_resolves() {
        let ix = declared_intersection();
        assert_eq!(ix.format_version, 2);
        ix.validate_against_baselines(openai_inventory(), openrouter_inventory())
            .expect("intersection validates");
    }

    #[test]
    fn members_are_coding_agent_verified_only() {
        let keys: HashSet<_> = declared_intersection()
            .members
            .iter()
            .map(|m| m.method_path_key())
            .collect();
        assert!(keys.contains("POST /chat/completions"));
        assert!(keys.contains("POST /responses"));
        assert!(keys.contains("POST /embeddings"));
        assert!(keys.contains("GET /models"));
        // Files/media must not be claimed as verified intersection.
        assert!(!keys.iter().any(|k| k.contains("/files")));
        assert!(!keys.iter().any(|k| k.contains("/audio")));
        assert!(!keys.iter().any(|k| k.contains("/videos")));
    }

    #[test]
    fn every_member_has_semantic_evidence() {
        for m in &declared_intersection().members {
            assert!(!m.semantic_evidence.rationale.is_empty());
            assert!(
                m.semantic_evidence.openai_schema.is_some()
                    || m.semantic_evidence.openrouter_schema.is_some()
                    || !m.semantic_evidence.rationale.is_empty()
            );
        }
    }

    #[test]
    fn openrouter_three_categories_partition_baseline() {
        let ix = declared_intersection();
        let or = openrouter_inventory().endpoint_map();
        let mut covered = HashSet::new();
        for m in &ix.members {
            assert!(covered.insert(m.method_path_key()));
        }
        for n in &ix.same_path_unverified_overlap {
            assert!(covered.insert(n.method_path_key()));
        }
        let outside = if ix.openrouter_contract_outside_intersection.is_empty() {
            &ix.openrouter_native_operations
        } else {
            &ix.openrouter_contract_outside_intersection
        };
        for n in outside {
            assert!(covered.insert(n.method_path_key()));
        }
        assert_eq!(covered.len(), or.len());
        for k in or.keys() {
            assert!(covered.contains(k), "missing {k}");
        }
    }

    #[test]
    fn bindings_are_not_implemented() {
        let ix = declared_intersection();
        for m in &ix.members {
            assert_eq!(m.client_binding(), BindingStatus::NotImplemented);
            assert_eq!(m.cli_binding(), BindingStatus::NotImplemented);
        }
    }

    #[test]
    fn client_completeness_claims_are_not_supported() {
        let claims = declared_intersection().openai_client_completeness_claims();
        assert!(!claims.is_empty());
        for c in claims {
            assert_ne!(c.status, CompatibilityStatus::Supported);
            assert_eq!(c.client_binding, BindingStatus::NotImplemented);
            assert_eq!(c.surface, ClaimSurface::OpenaiClientCompleteness);
            claim_is_consistent(&c).unwrap();
        }
    }

    #[test]
    fn baseline_presence_claims_are_supported_without_client_coverage() {
        let claims = declared_intersection().openai_baseline_presence_claims();
        assert!(!claims.is_empty());
        for c in claims {
            assert_eq!(c.status, CompatibilityStatus::Supported);
            assert_eq!(c.surface, ClaimSurface::OpenaiBaselinePresence);
            assert_eq!(c.client_binding, BindingStatus::NotImplemented);
            claim_is_consistent(&c).unwrap();
        }
    }

    #[test]
    fn report_json_serializes() {
        let report = intersection_report_json();
        assert_eq!(report["member_count"], 4);
        assert!(report["same_path_unverified_count"].as_u64().unwrap() >= 1);
    }
}
