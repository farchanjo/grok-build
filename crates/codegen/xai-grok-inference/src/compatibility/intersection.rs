//! Declared OpenAI ↔ OpenRouter intersection inventory (semantic).

use super::domain::{
    ApiFamily, BindingStatus, ClaimSurface, CompatibilityStatus, Evidence, EvidenceKind,
    HttpMethod, OperationClaim, OperationIdentity, Transport, claim_is_consistent,
    media_type_is_valid, path_is_safe, sha256_hex_is_valid, source_revision_is_valid,
    timestamp_is_rfc3339_utc,
};
use super::inventory::{InventoryEndpoint, ProviderInventory};
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
        let mut seen = HashSet::new();
        for raw in &self.transports {
            let t = Transport::parse_strict(raw)
                .ok_or_else(|| format!("invalid transport `{raw}` on {}", self.shared_id))?;
            if !seen.insert(t) {
                return Err(format!(
                    "duplicate transport `{}` on {}",
                    t.as_str(),
                    self.shared_id
                ));
            }
            transports.push(t);
        }
        if transports.is_empty() {
            return Err(format!("empty transports for {}", self.shared_id));
        }
        for mt in self
            .request_content_types
            .iter()
            .chain(self.response_content_types.iter())
        {
            if !media_type_is_valid(mt) {
                return Err(format!("malformed media type `{mt}` on {}", self.shared_id));
            }
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
///
/// OpenRouter baseline partition (single source of truth per category):
/// 1. `members` — verified semantic intersection (common transport/content subset)
/// 2. `same_path_unverified_overlap` — METHOD+path on both sides, semantics not verified
/// 3. `openrouter_contract_outside_intersection` — path exclusive to OpenRouter
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DeclaredIntersection {
    pub format_version: u32,
    pub kind: String,
    pub generated_at_utc: String,
    pub openai_baseline: IntersectionBaselineRef,
    pub openrouter_baseline: IntersectionBaselineRef,
    pub members: Vec<IntersectionMember>,
    #[serde(default)]
    pub same_path_unverified_overlap: Vec<CategorizedOpenrouterOp>,
    #[serde(default)]
    pub openrouter_contract_outside_intersection: Vec<CategorizedOpenrouterOp>,
    #[serde(default)]
    pub notes: Vec<String>,
}

fn sorted_set(items: &[String]) -> BTreeSet<String> {
    items.iter().cloned().collect()
}

fn endpoint_transport_set(ep: &InventoryEndpoint) -> Result<BTreeSet<String>, String> {
    let mut labels = ep.transports.clone();
    if labels.is_empty()
        && let Some(t) = &ep.transport
    {
        labels.push(t.clone());
    }
    let mut out = BTreeSet::new();
    for raw in labels {
        Transport::parse_strict(&raw)
            .ok_or_else(|| format!("invalid transport `{raw}` on {}", ep.method_path_key()))?;
        out.insert(raw);
    }
    Ok(out)
}

/// Declared member contract must equal the set-intersection of both vendor ops.
fn validate_common_contract(
    member: &IntersectionMember,
    oa: &InventoryEndpoint,
    ore: &InventoryEndpoint,
) -> Result<(), String> {
    let key = member.method_path_key();
    let oa_t = endpoint_transport_set(oa)?;
    let or_t = endpoint_transport_set(ore)?;
    let common_t: BTreeSet<_> = oa_t.intersection(&or_t).cloned().collect();
    let declared_t = sorted_set(&member.transports);
    if declared_t != common_t {
        return Err(format!(
            "member {key} transports must equal vendor intersection: declared={declared_t:?} common={common_t:?}"
        ));
    }
    if declared_t.is_empty() {
        return Err(format!("member {key} has empty common transport set"));
    }

    let oa_req = sorted_set(&oa.request_content_types);
    let or_req = sorted_set(&ore.request_content_types);
    let common_req: BTreeSet<_> = oa_req.intersection(&or_req).cloned().collect();
    let declared_req = sorted_set(&member.request_content_types);
    if declared_req != common_req {
        return Err(format!(
            "member {key} request_content_types must equal vendor intersection: declared={declared_req:?} common={common_req:?}"
        ));
    }

    let oa_resp = sorted_set(&oa.response_content_types);
    let or_resp = sorted_set(&ore.response_content_types);
    let common_resp: BTreeSet<_> = oa_resp.intersection(&or_resp).cloned().collect();
    let declared_resp = sorted_set(&member.response_content_types);
    if declared_resp != common_resp {
        return Err(format!(
            "member {key} response_content_types must equal vendor intersection: declared={declared_resp:?} common={common_resp:?}"
        ));
    }

    // Declared values must also be subsets of each vendor individually.
    if !declared_t.is_subset(&oa_t) || !declared_t.is_subset(&or_t) {
        return Err(format!(
            "member {key} transports not subset of both vendors"
        ));
    }
    if !declared_req.is_subset(&oa_req) || !declared_req.is_subset(&or_req) {
        return Err(format!(
            "member {key} request types not subset of both vendors"
        ));
    }
    if !declared_resp.is_subset(&oa_resp) || !declared_resp.is_subset(&or_resp) {
        return Err(format!(
            "member {key} response types not subset of both vendors"
        ));
    }
    Ok(())
}

impl DeclaredIntersection {
    /// Path-exclusive OpenRouter operations (single serialized source of truth).
    pub fn openrouter_path_exclusive_ops(&self) -> &[CategorizedOpenrouterOp] {
        &self.openrouter_contract_outside_intersection
    }

    pub fn validate_against_baselines(
        &self,
        openai: &ProviderInventory,
        openrouter: &ProviderInventory,
    ) -> Result<(), String> {
        if self.format_version < 2 {
            return Err("intersection format_version must be >= 2".into());
        }
        if !timestamp_is_rfc3339_utc(&self.generated_at_utc) {
            return Err(format!(
                "invalid generated_at_utc `{}`",
                self.generated_at_utc
            ));
        }
        if !sha256_hex_is_valid(&self.openai_baseline.content_sha256)
            || !sha256_hex_is_valid(&self.openrouter_baseline.content_sha256)
        {
            return Err("intersection baseline content_sha256 invalid".into());
        }
        if !timestamp_is_rfc3339_utc(&self.openai_baseline.fetched_at_utc)
            || !timestamp_is_rfc3339_utc(&self.openrouter_baseline.fetched_at_utc)
        {
            return Err("intersection baseline fetched_at_utc invalid".into());
        }
        if let Some(rev) = &self.openai_baseline.source_revision
            && !source_revision_is_valid(rev)
        {
            return Err("intersection openai source_revision invalid".into());
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
            if !timestamp_is_rfc3339_utc(&m.evidence.timestamp_utc) {
                return Err(format!("member {} evidence timestamp invalid", m.shared_id));
            }
            if !sha256_hex_is_valid(&m.evidence.openai_content_sha256)
                || !sha256_hex_is_valid(&m.evidence.openrouter_content_sha256)
            {
                return Err(format!("member {} evidence sha invalid", m.shared_id));
            }
            if let Some(rev) = &m.evidence.openai_source_revision
                && !source_revision_is_valid(rev)
            {
                return Err(format!(
                    "member {} evidence source_revision invalid",
                    m.shared_id
                ));
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
            validate_common_contract(m, oa, ore)?;
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

        let mut exclusive_keys = HashSet::new();
        for n in self.openrouter_path_exclusive_ops() {
            let key = n.method_path_key();
            if !exclusive_keys.insert(key.clone()) {
                return Err(format!("duplicate path-exclusive op {key}"));
            }
            if oa_map.contains_key(&key) {
                return Err(format!(
                    "path-exclusive {key} also exists in OpenAI (use same-path-unverified)"
                ));
            }
            if !or_map.contains_key(&key) {
                return Err(format!(
                    "path-exclusive {key} missing from OpenRouter baseline"
                ));
            }
        }

        if !ix_keys.is_disjoint(&unv_keys)
            || !ix_keys.is_disjoint(&exclusive_keys)
            || !unv_keys.is_disjoint(&exclusive_keys)
        {
            return Err("OpenRouter category sets are not disjoint".into());
        }
        let covered: HashSet<String> = ix_keys
            .iter()
            .chain(unv_keys.iter())
            .chain(exclusive_keys.iter())
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

    /// Claims for verified intersection members only (not the full OpenAI ledger).
    pub fn verified_intersection_baseline_presence_claims(&self) -> Vec<OperationClaim> {
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

    /// Client-completeness claims for verified intersection members only.
    /// Always Unknown/NotImplemented in Change 4.
    pub fn verified_intersection_client_completeness_claims(&self) -> Vec<OperationClaim> {
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
        "openrouter_path_exclusive_count": ix.openrouter_path_exclusive_ops().len(),
        "members": ix.members.iter().map(|m| serde_json::json!({
            "shared_id": m.shared_id,
            "method_path": m.method_path_key(),
            "openai_operation_id": m.openai_operation_id,
            "openrouter_operation_id": m.openrouter_operation_id,
            "api_family": m.api_family,
            "transports": m.transports,
            "request_content_types": m.request_content_types,
            "response_content_types": m.response_content_types,
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
        assert!(!keys.iter().any(|k| k.contains("/files")));
        assert!(!keys.iter().any(|k| k.contains("/audio")));
        assert!(!keys.iter().any(|k| k.contains("/videos")));
    }

    #[test]
    fn member_transports_are_common_subset_not_union() {
        let ix = declared_intersection();
        let oa = openai_inventory().endpoint_map();
        let or = openrouter_inventory().endpoint_map();
        for m in &ix.members {
            let key = m.method_path_key();
            let oa_e = oa.get(&key).unwrap();
            let or_e = or.get(&key).unwrap();
            validate_common_contract(m, oa_e, or_e).unwrap();
        }
        // Embeddings specifically: OpenRouter has SSE, OpenAI does not → common JSON only.
        let emb = ix
            .members
            .iter()
            .find(|m| m.method_path_key() == "POST /embeddings")
            .unwrap();
        assert_eq!(emb.transports, vec!["http_json".to_string()]);
        assert!(!emb.transports.iter().any(|t| t == "http_sse"));
    }

    #[test]
    fn union_overclaim_is_rejected() {
        let oa = openai_inventory().endpoint_map();
        let or = openrouter_inventory().endpoint_map();
        let key = "POST /embeddings";
        let mut m = declared_intersection()
            .members
            .iter()
            .find(|x| x.method_path_key() == key)
            .unwrap()
            .clone();
        // Fabricate a union overclaim (SSE only on OpenRouter side).
        m.transports = vec!["http_json".into(), "http_sse".into()];
        let err = validate_common_contract(&m, oa[key], or[key]).unwrap_err();
        assert!(
            err.contains("must equal vendor intersection"),
            "unexpected: {err}"
        );
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
        for n in ix.openrouter_path_exclusive_ops() {
            assert!(covered.insert(n.method_path_key()));
        }
        assert_eq!(covered.len(), or.len());
        for k in or.keys() {
            assert!(covered.contains(k), "missing {k}");
        }
    }

    #[test]
    fn no_duplicated_native_operations_field() {
        // Serialized JSON must not reintroduce the duplicate alias.
        let raw = include_str!("../../baselines/intersection/declared_intersection.json");
        assert!(
            !raw.contains("\"openrouter_native_operations\""),
            "duplicate openrouter_native_operations must not be serialized"
        );
    }

    #[test]
    fn verified_intersection_client_claims_implemented() {
        // Intersection members share OpenAI client completeness (Implemented).
        // This helper still documents Change-4-era NotImplemented for the
        // *intersection JSON* itself; live OpenAI inventory claims are Implemented.
        let claims = declared_intersection().verified_intersection_client_completeness_claims();
        assert_eq!(claims.len(), 4);
        for c in claims {
            claim_is_consistent(&c).unwrap();
        }
    }

    #[test]
    fn full_openai_claim_ledgers_cover_every_endpoint() {
        let inv = openai_inventory();
        let presence = inv.baseline_presence_claims().unwrap();
        let complete = inv.client_completeness_claims().unwrap();
        assert_eq!(presence.len() as u64, inv.baseline.endpoint_count);
        assert_eq!(complete.len() as u64, inv.baseline.endpoint_count);
        assert_eq!(presence.len(), 287);
        let mut pkeys = HashSet::new();
        let mut ckeys = HashSet::new();
        for c in &presence {
            assert_eq!(c.surface, ClaimSurface::OpenaiBaselinePresence);
            assert_eq!(c.status, CompatibilityStatus::Supported);
            assert!(pkeys.insert(c.identity.method_path_key()));
        }
        for c in &complete {
            assert_eq!(c.surface, ClaimSurface::OpenaiClientCompleteness);
            assert_eq!(c.status, CompatibilityStatus::Supported);
            assert_eq!(c.client_binding, BindingStatus::Implemented);
            assert_eq!(c.cli_binding, BindingStatus::Implemented);
            assert!(ckeys.insert(c.identity.method_path_key()));
        }
        assert_eq!(pkeys, ckeys);
        for ep in &inv.endpoints {
            assert!(pkeys.contains(&ep.method_path_key()));
        }
    }

    #[test]
    fn report_json_serializes() {
        let report = intersection_report_json();
        assert_eq!(report["member_count"], 4);
        assert!(report["same_path_unverified_count"].as_u64().unwrap() >= 1);
        assert_eq!(report["openrouter_path_exclusive_count"], 77);
    }
}
