//! OpenAI / OpenRouter compatibility baselines and contracts (Change 4).
//!
//! # Inventories
//!
//! - Official OpenAI baseline (`baselines/openai/`) — commit-pinned
//! - Official OpenRouter baseline (`baselines/openrouter/`)
//! - Declared **semantic** intersection (`baselines/intersection/`)
//!
//! # Claims policy
//!
//! OpenRouter is **not** the full OpenAI platform. Intersection membership
//! requires schema/document evidence, not METHOD+path alone. Member
//! transports/content types are the **set-intersection** of both vendor ops.
//! Typed client/CLI bindings are `NotImplemented` until later milestones;
//! full OpenAI client-completeness claims remain `Unknown` for all 287 ops.
//!
//! See `docs/compatibility-baselines.md` and each inventory's `PROVENANCE.md`.

pub mod domain;
pub mod intersection;
pub mod inventory;

pub use domain::{
    ApiFamily, BindingStatus, ClaimSurface, CompatibilityStatus, Evidence, EvidenceKind,
    HttpMethod, OperationClaim, OperationIdentity, Transport, claim_is_consistent,
    media_type_is_valid, path_is_safe, sha256_hex_is_valid, source_revision_is_valid,
    timestamp_is_rfc3339_utc,
};
pub use intersection::{
    DeclaredIntersection, IntersectionMember, declared_intersection, intersection_report_json,
};
pub use inventory::{
    BaselineMeta, InventoryEndpoint, ProviderInventory, inventory_report_json, openai_inventory,
    openrouter_inventory,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompatibilityCounts {
    pub openai_endpoints: u64,
    pub openai_paths: u64,
    pub openrouter_endpoints: u64,
    pub openrouter_paths: u64,
    pub intersection_members: usize,
    pub same_path_unverified: usize,
    pub openrouter_path_exclusive: usize,
}

pub fn compatibility_counts() -> CompatibilityCounts {
    let oa = openai_inventory();
    let or = openrouter_inventory();
    let ix = declared_intersection();
    CompatibilityCounts {
        openai_endpoints: oa.baseline.endpoint_count,
        openai_paths: oa.baseline.path_count,
        openrouter_endpoints: or.baseline.endpoint_count,
        openrouter_paths: or.baseline.path_count,
        intersection_members: ix.members.len(),
        same_path_unverified: ix.same_path_unverified_overlap.len(),
        openrouter_path_exclusive: ix.openrouter_path_exclusive_ops().len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn counts_partition_openrouter() {
        let c = compatibility_counts();
        assert_eq!(c.intersection_members, 4);
        assert_eq!(
            (c.intersection_members + c.same_path_unverified + c.openrouter_path_exclusive) as u64,
            c.openrouter_endpoints
        );
    }

    #[test]
    fn enumerates_every_endpoint_in_both_baselines() {
        for (name, inv) in [
            ("openai", openai_inventory()),
            ("openrouter", openrouter_inventory()),
        ] {
            inv.validate_integrity()
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            for ep in &inv.endpoints {
                assert!(path_is_safe(&ep.path));
                ep.to_identity().unwrap_or_else(|e| panic!("{name}: {e}"));
            }
        }
    }

    #[test]
    fn openai_full_claim_ledger_sizes() {
        let inv = openai_inventory();
        let presence = inv.baseline_presence_claims().unwrap();
        let complete = inv.client_completeness_claims().unwrap();
        assert_eq!(presence.len(), inv.endpoints.len());
        assert_eq!(complete.len(), inv.endpoints.len());
        assert_eq!(presence.len() as u64, 287);
        assert!(
            complete
                .iter()
                .all(|c| c.status == CompatibilityStatus::Unknown)
        );
        assert!(
            presence
                .iter()
                .all(|c| c.status == CompatibilityStatus::Supported)
        );
    }

    #[test]
    fn openai_generator_runs_on_mini_fixture() {
        let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let generator = crate_dir.join("baselines/openai/generate_inventory.py");
        let mini = crate_dir.join("baselines/openai/fixtures/mini_openapi.json");
        let mut out = std::env::temp_dir();
        out.push(format!("openai-mini-inv-{}.json", std::process::id()));
        let raw = std::fs::read(&mini).unwrap();
        let sha = sha256_hex(&raw);
        let status = Command::new("python3")
            .arg(&generator)
            .arg("--input")
            .arg(&mini)
            .arg("--output")
            .arg(&out)
            .arg("--fetched-at-utc")
            .arg("2026-07-25T16:25:32Z")
            .arg("--expect-source-sha256")
            .arg(&sha)
            .arg("--expect-source-bytes")
            .arg(raw.len().to_string())
            .status()
            .expect("spawn openai generator");
        assert!(status.success(), "openai generator failed on mini fixture");
        let inv: ProviderInventory =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(inv.provider, "openai");
        assert_eq!(inv.format_version, 2);
        let _ = std::fs::remove_file(&out);
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("shasum")
            .arg("-a")
            .arg("256")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("shasum");
        child.stdin.as_mut().unwrap().write_all(bytes).unwrap();
        let out = child.wait_with_output().unwrap();
        let s = String::from_utf8_lossy(&out.stdout);
        s.split_whitespace().next().unwrap().to_owned()
    }
}
