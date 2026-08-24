//! Frozen additive ACP/JSON and mixed-version fixtures for skills.

use super::author::text_leaks_secrets;
use super::management::{
    SKILLS_API_VERSION, SkillsListV1Response, SkillsRegressStatusResponse, SkillsValidateResponse,
    SkillsVersionedRequest, require_api_version,
};
use super::status::SkillHealthStatus;

fn load(name: &str) -> String {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/implementations/skills/strict/testdata/acp")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {name}: {err}"))
}

fn assert_secret_free(name: &str, text: &str) {
    if let Some(token) = text_leaks_secrets(text) {
        panic!("{name} leaked {token}");
    }
    assert!(
        !text.contains("/Users/"),
        "{name} must not contain absolute home paths"
    );
}

#[test]
fn frozen_list_v1_is_additive_and_secret_free() {
    let raw = load("skills_list_v1.json");
    assert_secret_free("skills_list_v1.json", &raw);
    let parsed: SkillsListV1Response = serde_json::from_str(&raw).unwrap();
    assert_eq!(parsed.api_version, SKILLS_API_VERSION);
    assert_eq!(parsed.generation, 4);
    assert_eq!(parsed.health.valid_pass, 1);
    assert_eq!(parsed.health.quarantined, 1);
    assert_eq!(parsed.skills.len(), 2);
    assert!(parsed.skills[0].enableable);
    assert!(!parsed.skills[1].enableable);
    assert_eq!(parsed.skills[1].status, SkillHealthStatus::Quarantined);
    assert!(parsed.skills[1].skill.is_none());
    let round = serde_json::to_string(&parsed).unwrap();
    assert_secret_free("skills_list_v1 roundtrip", &round);
    assert!(
        !round.contains("futureField"),
        "unknown additive fields must not be required on the write path"
    );
}

#[test]
fn frozen_list_v0_keeps_historical_shape() {
    let raw = load("skills_list_v0.json");
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert!(value.get("apiVersion").is_none());
    assert!(value.get("skills").and_then(|s| s.as_array()).is_some());
}

#[test]
fn frozen_validate_and_regress_status_are_versioned() {
    let validate: SkillsValidateResponse =
        serde_json::from_str(&load("skills_validate_v1.json")).unwrap();
    assert_eq!(validate.api_version, 1);
    assert_eq!(validate.status, SkillHealthStatus::Quarantined);
    assert!(!validate.diagnostics.is_empty());
    assert_secret_free("skills_validate_v1.json", &load("skills_validate_v1.json"));

    let status: SkillsRegressStatusResponse =
        serde_json::from_str(&load("skills_regress_status_v1.json")).unwrap();
    assert_eq!(status.api_version, 1);
    assert!(!status.running);
    assert_eq!(
        status.summary.as_ref().map(|s| s.status),
        Some(SkillHealthStatus::Stale)
    );
}

#[test]
fn missing_and_unsupported_versions_fail_closed() {
    let missing: SkillsVersionedRequest =
        serde_json::from_str(&load("skills_validate_missing_version.json")).unwrap();
    assert!(require_api_version(missing.api_version).is_err());
    assert!(require_api_version(Some(0)).is_err());
    assert!(require_api_version(Some(2)).is_err());
    assert_eq!(require_api_version(Some(1)).unwrap(), 1);
}

#[test]
fn search_fixture_is_names_only() {
    let raw = load("skills_search_v1.json");
    assert_secret_free("skills_search_v1.json", &raw);
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(value["apiVersion"], 1);
    assert!(value["degraded"].as_bool().unwrap());
    let names = value["names"].as_array().unwrap();
    assert_eq!(names.len(), 2);
    assert!(value.get("bodies").is_none());
    assert!(value.get("paths").is_none());
    assert!(value.get("vectors").is_none());
}
