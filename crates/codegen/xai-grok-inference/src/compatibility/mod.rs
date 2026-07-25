//! OpenAI / OpenRouter compatibility baselines and contracts (Change 4).
//!
//! # Inventories
//!
//! - Official OpenAI baseline (`baselines/openai/`)
//! - Official OpenRouter baseline (`baselines/openrouter/`) — reused from
//!   earlier milestones
//! - Declared intersection (`baselines/intersection/`)
//!
//! # Claims policy
//!
//! OpenRouter is **not** the full OpenAI platform. OpenRouter-native operations
//! are tracked separately. Typed client/CLI bindings are `NotImplemented` until
//! later milestones; this module records that honesty rather than placeholders
//! that look like coverage.
//!
//! See `docs/compatibility-baselines.md` and each inventory's `PROVENANCE.md`.

pub mod domain;
pub mod intersection;
pub mod inventory;

pub use domain::{
    ApiFamily, BindingStatus, ClaimSurface, CompatibilityStatus, Evidence, EvidenceKind,
    HttpMethod, OperationClaim, OperationIdentity, Transport, path_is_safe,
};
pub use intersection::{
    DeclaredIntersection, IntersectionMember, OpenrouterNativeOp, declared_intersection,
    intersection_report_json,
};
pub use inventory::{
    BaselineMeta, InventoryEndpoint, ProviderInventory, inventory_report_json, openai_inventory,
    openrouter_inventory,
};

/// Counts derived from the pinned inventories (for docs/tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompatibilityCounts {
    pub openai_endpoints: u64,
    pub openai_paths: u64,
    pub openrouter_endpoints: u64,
    pub openrouter_paths: u64,
    pub intersection_members: usize,
    pub openrouter_native_operations: usize,
}

/// Live counts from embedded inventories.
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
        openrouter_native_operations: ix.openrouter_native_operations.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn counts_are_positive_and_consistent() {
        let c = compatibility_counts();
        assert!(c.openai_endpoints > 0);
        assert!(c.openrouter_endpoints > 0);
        assert!(c.intersection_members > 0);
        assert!(c.openrouter_native_operations > 0);
        // Native + intersection method+paths should not exceed OpenRouter total.
        assert!(
            (c.intersection_members as u64) + (c.openrouter_native_operations as u64)
                <= c.openrouter_endpoints
        );
    }

    #[test]
    fn openai_generator_runs_on_mini_fixture() {
        let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let generator = crate_dir.join("baselines/openai/generate_inventory.py");
        let mini = crate_dir.join("baselines/openai/fixtures/mini_openapi.json");
        let mut out = std::env::temp_dir();
        out.push(format!("openai-mini-inv-{}.json", std::process::id()));
        let status = Command::new("python3")
            .arg(&generator)
            .arg("--input")
            .arg(&mini)
            .arg("--output")
            .arg(&out)
            .arg("--fetched-at-utc")
            .arg("2026-07-25T00:00:00Z")
            .arg("--source-sha256")
            .arg("00".repeat(32))
            .arg("--source-bytes")
            .arg("1")
            .arg("--source-format")
            .arg("json")
            .status()
            .expect("spawn openai generator");
        assert!(status.success(), "openai generator failed on mini fixture");
        let inv: ProviderInventory =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(inv.provider, "openai");
        inv.validate_integrity().expect("mini integrity");
        let _ = std::fs::remove_file(&out);
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
                assert!(path_is_safe(&ep.path), "{name} unsafe path {}", ep.path);
                assert!(
                    HttpMethod::parse(&ep.method).is_some(),
                    "{name} bad method {}",
                    ep.method
                );
                // Transport is free-form in older OpenRouter inventory; parse never panics.
                let _ = ep.transport();
            }
        }
    }
}
