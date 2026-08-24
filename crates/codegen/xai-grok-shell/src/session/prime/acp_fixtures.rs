//! Frozen additive ACP/JSON and mixed-version fixtures for Prime index ops.

use xai_grok_tools::implementations::skills::strict::{require_api_version, text_leaks_secrets};

use super::ops::{
    PrimeIndexCapabilities, PrimeIndexJobStatus, PrimeIndexStatus, PrimeIndexUpdate,
    parse_prime_index_available,
};

fn load(name: &str) -> String {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/session/prime/testdata/acp")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {name}: {err}"))
}

fn assert_secret_free(name: &str, text: &str) {
    if let Some(token) = text_leaks_secrets(text) {
        panic!("{name} leaked {token}");
    }
    for leak in ["sk-", "BODY", "vector_values", "abcdef0123456789ffff"] {
        assert!(!text.contains(leak), "{name} leaked {leak}");
    }
}

#[test]
fn frozen_prime_status_is_additive_and_secret_free() {
    let raw = load("prime_index_status_v1.json");
    assert_secret_free("prime_index_status_v1.json", &raw);
    let parsed: PrimeIndexStatus = serde_json::from_str(&raw).unwrap();
    assert_eq!(parsed.api_version, 1);
    assert_eq!(parsed.generation, 4);
    assert_eq!(parsed.skills.collection, "skills");
    assert_eq!(parsed.agents.collection, "agents");
    assert_eq!(parsed.configured_route.as_deref(), Some("main"));
    assert_eq!(parsed.capabilities, PrimeIndexCapabilities::SUPPORTED);
    let round = serde_json::to_string(&parsed).unwrap();
    assert_secret_free("prime status roundtrip", &round);
    assert!(!round.contains("futureField"));
    assert!(!round.contains("\"fingerprint\""));
}

#[test]
fn frozen_job_and_update_ignore_unknown_fields() {
    let job: PrimeIndexJobStatus = serde_json::from_str(&load("prime_index_job_v1.json")).unwrap();
    assert_eq!(job.state, "failed");
    assert_eq!(job.failure.as_deref(), Some("embed_failed"));
    assert!(!job.confirm_configured_profile);
    assert_secret_free("prime_index_job_v1.json", &load("prime_index_job_v1.json"));

    let update: PrimeIndexUpdate =
        serde_json::from_str(&load("prime_index_update_v1.json")).unwrap();
    assert_eq!(update.notify_seq, 9);
    assert_eq!(update.changed_fields, vec!["job"]);
    assert!(update.job.is_none());
}

#[test]
fn mixed_version_prime_defaults_fail_closed_or_unsupported() {
    assert!(require_api_version(None).is_err());
    let legacy: PrimeIndexUpdate =
        serde_json::from_str(&load("prime_index_legacy_missing.json")).unwrap();
    assert_eq!(legacy.generation, 0);
    assert_eq!(legacy.notify_seq, 0);
    assert_eq!(
        parse_prime_index_available(None),
        PrimeIndexCapabilities::UNSUPPORTED
    );
    let old_shell = serde_json::json!({"sessionRecap": true});
    assert_eq!(
        parse_prime_index_available(Some(&old_shell)),
        PrimeIndexCapabilities::UNSUPPORTED
    );
}
