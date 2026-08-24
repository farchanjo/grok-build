//! Machine-wide Prime index job notifications.
//!
//! After status/job changes, emit `x.ai/prime/index/update` with only
//! bounded secret-free fields: `schemaVersion`, `apiVersion`, `generation`,
//! `notifySeq`, `fingerprintShort`, optional job, `changedFields`.
//!
//! Delivery mirrors retrieval notify:
//! 1. Atomic notify file under `$GROK_HOME/state/prime_index_notify.json`
//! 2. Multi-subscriber gateway fanout
//! 3. File-poll for TUI / `--no-leader` with independent cursors keyed by
//!    `notifySeq` (job ticks reuse inventory `generation`)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use agent_client_protocol as acp;

use super::ops::{
    PRIME_INDEX_API_VERSION, PRIME_INDEX_SCHEMA_VERSION, PrimeIndexJobStatus, PrimeIndexUpdate,
};

type GatewayFn = std::sync::Arc<dyn Fn(acp::ExtNotification) + Send + Sync>;

static GATEWAYS: OnceLock<Mutex<Vec<GatewayFn>>> = OnceLock::new();
static POLL_STATE: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();
static NOTIFY_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_notify_seq() -> u64 {
    NOTIFY_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn gateways() -> &'static Mutex<Vec<GatewayFn>> {
    GATEWAYS.get_or_init(|| Mutex::new(Vec::new()))
}

fn poll_state() -> &'static Mutex<HashMap<PathBuf, u64>> {
    POLL_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn set_prime_index_update_forwarder(
    forward: Option<Box<dyn Fn(acp::ExtNotification) + Send + Sync>>,
) {
    if let Ok(mut slot) = gateways().lock() {
        slot.clear();
        if let Some(f) = forward {
            slot.push(std::sync::Arc::from(f));
        }
    }
}

pub fn register_prime_index_update_forwarder(
    forward: Box<dyn Fn(acp::ExtNotification) + Send + Sync>,
) {
    if let Ok(mut slot) = gateways().lock() {
        slot.push(std::sync::Arc::from(forward));
    }
}

pub fn clear_prime_index_update_forwarders() {
    if let Ok(mut slot) = gateways().lock() {
        slot.clear();
    }
}

pub fn try_forward_prime_index_update(params: &serde_json::Value) {
    let snapshot: Vec<GatewayFn> = {
        let Ok(slot) = gateways().lock() else {
            return;
        };
        slot.iter().cloned().collect()
    };
    for fwd in snapshot {
        let Ok(raw) = serde_json::value::to_raw_value(params) else {
            continue;
        };
        let notif = acp::ExtNotification::new("x.ai/prime/index/update", raw.into());
        fwd(notif);
    }
}

pub fn prime_index_update_params(update: &PrimeIndexUpdate) -> serde_json::Value {
    serde_json::to_value(update).unwrap_or_else(|_| {
        serde_json::json!({
            "schemaVersion": PRIME_INDEX_SCHEMA_VERSION,
            "apiVersion": PRIME_INDEX_API_VERSION,
            "generation": update.generation,
            "notifySeq": update.notify_seq,
        })
    })
}

pub fn notify_file_path(home: &Path) -> PathBuf {
    home.join("state/prime_index_notify.json")
}

pub fn publish_prime_index_update(home: &Path, update: &PrimeIndexUpdate) {
    let mut owned = update.clone();
    if owned.notify_seq == 0 {
        owned.notify_seq = next_notify_seq();
    }
    let params = prime_index_update_params(&owned);
    let path = notify_file_path(home);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    if let Ok(bytes) = serde_json::to_vec_pretty(&params)
        && std::fs::write(&tmp, &bytes).is_ok()
    {
        let _ = std::fs::rename(&tmp, &path);
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
    try_forward_prime_index_update(&params);
}

pub fn publish_job_update(
    home: &Path,
    generation: u64,
    fingerprint_short: &str,
    job: Option<PrimeIndexJobStatus>,
    changed_fields: &[&str],
) {
    let update = PrimeIndexUpdate {
        schema_version: PRIME_INDEX_SCHEMA_VERSION,
        api_version: Some(PRIME_INDEX_API_VERSION),
        generation,
        notify_seq: 0,
        fingerprint_short: fingerprint_short.to_owned(),
        job,
        changed_fields: changed_fields.iter().map(|s| (*s).to_owned()).collect(),
    };
    publish_prime_index_update(home, &update);
}

pub fn poll_notify_file_if_newer(home: &Path) -> Option<serde_json::Value> {
    let home_key = home.to_path_buf();
    let mut cursor = poll_state()
        .lock()
        .ok()
        .and_then(|map| map.get(&home_key).copied())
        .unwrap_or(0);
    let val = poll_notify_file_with_cursor(home, &mut cursor)?;
    if let Ok(mut map) = poll_state().lock() {
        map.insert(home_key, cursor);
    }
    Some(val)
}

pub fn poll_notify_file_with_cursor(home: &Path, last_seen: &mut u64) -> Option<serde_json::Value> {
    let path = notify_file_path(home);
    let raw = std::fs::read_to_string(&path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let generation = val.get("generation").and_then(|g| g.as_u64()).unwrap_or(0);
    let notify_seq = val.get("notifySeq").and_then(|g| g.as_u64()).unwrap_or(0);
    let has_job = val.get("job").is_some_and(|j| !j.is_null());
    // Prefer notifySeq so same-generation job ticks are not coalesced. Fall
    // back to inventory generation for old shells. Generation 0 is still
    // delivered while a job is present (empty inventory during first backfill).
    let token = if notify_seq > 0 {
        notify_seq
    } else if generation > 0 {
        generation
    } else if has_job {
        1
    } else {
        return None;
    };
    if token <= *last_seen {
        return None;
    }
    *last_seen = token;
    Some(val)
}

/// Drop the process-global poll cursor for `home` so reconnect can re-read
/// the on-disk snapshot at the current generation.
pub fn reset_poll_cursor(home: &Path) {
    if let Ok(mut map) = poll_state().lock() {
        map.remove(&home.to_path_buf());
    }
}

#[cfg(test)]
pub fn reset_poll_state_for_tests() {
    if let Ok(mut map) = poll_state().lock() {
        map.clear();
    }
}

#[cfg(test)]
pub fn gateway_mutex_try_lock_ok_for_test() -> bool {
    match gateways().try_lock() {
        Ok(_) => true,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    fn sample_update(generation: u64) -> PrimeIndexUpdate {
        PrimeIndexUpdate {
            schema_version: PRIME_INDEX_SCHEMA_VERSION,
            api_version: Some(PRIME_INDEX_API_VERSION),
            generation,
            notify_seq: 0,
            fingerprint_short: "abc123def456".into(),
            job: None,
            changed_fields: vec!["job".into()],
        }
    }

    fn sample_job_update(generation: u64, done: u64, state: &str) -> PrimeIndexUpdate {
        PrimeIndexUpdate {
            schema_version: PRIME_INDEX_SCHEMA_VERSION,
            api_version: Some(PRIME_INDEX_API_VERSION),
            generation,
            notify_seq: 0,
            fingerprint_short: "abc123def456".into(),
            job: Some(PrimeIndexJobStatus {
                api_version: PRIME_INDEX_API_VERSION,
                job_id: "j1".into(),
                kind: "backfill".into(),
                collection: "skills".into(),
                state: state.into(),
                generation,
                fingerprint_short: "abc123def456".into(),
                done,
                total: 3,
                confirm_configured_profile: false,
                configured_route: None,
                failure: None,
            }),
            changed_fields: vec!["job".into()],
        }
    }

    #[test]
    #[serial]
    fn forwarder_receives_method() {
        clear_prime_index_update_forwarders();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();
        set_prime_index_update_forwarder(Some(Box::new(move |n| {
            assert_eq!(n.method.as_ref(), "x.ai/prime/index/update");
            hits2.fetch_add(1, Ordering::SeqCst);
        })));
        try_forward_prime_index_update(&prime_index_update_params(&sample_update(3)));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        clear_prime_index_update_forwarders();
    }

    #[test]
    #[serial]
    fn notify_file_poll_coalesces_and_rejects_stale() {
        reset_poll_state_for_tests();
        let dir = TempDir::new().unwrap();
        publish_prime_index_update(dir.path(), &sample_update(4));
        let first = poll_notify_file_if_newer(dir.path()).expect("first");
        assert_eq!(first["generation"], 4);
        assert!(poll_notify_file_if_newer(dir.path()).is_none());
        publish_prime_index_update(dir.path(), &sample_update(5));
        let second = poll_notify_file_if_newer(dir.path()).expect("second");
        assert_eq!(second["generation"], 5);
        let seq = second["notifySeq"].as_u64().unwrap();
        // Identical seq must not re-deliver even if the file is rewritten.
        let path = notify_file_path(dir.path());
        std::fs::write(&path, serde_json::to_vec(&second).unwrap()).unwrap();
        assert!(
            poll_notify_file_if_newer(dir.path()).is_none(),
            "same notifySeq must not re-deliver"
        );
        assert!(seq > 0, "publisher must stamp a notifySeq");
    }

    #[test]
    #[serial]
    fn same_generation_job_progress_redelivers_on_newer_seq() {
        reset_poll_state_for_tests();
        let dir = TempDir::new().unwrap();
        publish_prime_index_update(dir.path(), &sample_job_update(4, 1, "running"));
        let first = poll_notify_file_if_newer(dir.path()).expect("first job tick");
        assert_eq!(first["generation"], 4);
        assert_eq!(first["job"]["done"], 1);
        publish_prime_index_update(dir.path(), &sample_job_update(4, 2, "running"));
        let second = poll_notify_file_if_newer(dir.path()).expect("same-gen job tick");
        assert_eq!(second["generation"], 4);
        assert_eq!(second["job"]["done"], 2);
        assert!(
            second["notifySeq"].as_u64().unwrap() > first["notifySeq"].as_u64().unwrap(),
            "job ticks must advance notifySeq"
        );
        publish_prime_index_update(dir.path(), &sample_job_update(4, 3, "completed"));
        let third = poll_notify_file_if_newer(dir.path()).expect("completion tick");
        assert_eq!(third["job"]["state"], "completed");
    }

    #[test]
    #[serial]
    fn generation_zero_with_job_is_delivered() {
        reset_poll_state_for_tests();
        let dir = TempDir::new().unwrap();
        publish_prime_index_update(dir.path(), &sample_job_update(0, 1, "running"));
        let first = poll_notify_file_if_newer(dir.path()).expect("gen-0 job");
        assert_eq!(first["generation"], 0);
        assert_eq!(first["job"]["done"], 1);
        assert!(poll_notify_file_if_newer(dir.path()).is_none());
    }

    #[test]
    #[serial]
    fn reset_poll_cursor_redelivers_same_generation() {
        reset_poll_state_for_tests();
        let dir = TempDir::new().unwrap();
        publish_prime_index_update(dir.path(), &sample_update(7));
        let first = poll_notify_file_if_newer(dir.path()).expect("first");
        assert_eq!(first["generation"], 7);
        assert!(poll_notify_file_if_newer(dir.path()).is_none());
        reset_poll_cursor(dir.path());
        let again = poll_notify_file_if_newer(dir.path()).expect("reconnect catch-up");
        assert_eq!(again["generation"], 7);
        assert_eq!(again["notifySeq"], first["notifySeq"]);
    }

    #[test]
    #[serial]
    fn legacy_generation_zero_job_without_seq_is_delivered_once() {
        reset_poll_state_for_tests();
        let dir = TempDir::new().unwrap();
        let path = notify_file_path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"generation":0,"apiVersion":1,"job":{"apiVersion":1,"jobId":"j1","kind":"backfill","collection":"skills","state":"running","generation":0,"fingerprintShort":"abc","done":1,"total":3,"confirmConfiguredProfile":false}}"#,
        )
        .unwrap();
        let first = poll_notify_file_if_newer(dir.path()).expect("legacy gen-0 job");
        assert_eq!(first["job"]["done"], 1);
        assert!(poll_notify_file_if_newer(dir.path()).is_none());
    }

    #[test]
    #[serial]
    fn empty_or_zero_generation_is_ignored() {
        reset_poll_state_for_tests();
        let dir = TempDir::new().unwrap();
        let path = notify_file_path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"schemaVersion":1}"#).unwrap();
        assert!(poll_notify_file_if_newer(dir.path()).is_none());
        std::fs::write(&path, r#"{"generation":0}"#).unwrap();
        assert!(poll_notify_file_if_newer(dir.path()).is_none());
    }

    #[test]
    #[serial]
    fn forwarder_runs_after_gateway_mutex_release() {
        clear_prime_index_update_forwarders();
        let unlocked = Arc::new(AtomicUsize::new(0));
        let unlocked2 = unlocked.clone();
        register_prime_index_update_forwarder(Box::new(move |_n| {
            if gateway_mutex_try_lock_ok_for_test() {
                unlocked2.fetch_add(1, Ordering::SeqCst);
            }
        }));
        try_forward_prime_index_update(&prime_index_update_params(&sample_update(1)));
        assert_eq!(
            unlocked.load(Ordering::SeqCst),
            1,
            "forwarder must run after gateway mutex release"
        );
        clear_prime_index_update_forwarders();
    }

    #[test]
    fn update_payload_is_secret_free() {
        let json = serde_json::to_string(&sample_update(9)).unwrap();
        assert!(!json.contains("sk-"), "{json}");
        assert!(!json.contains("/Users/"), "{json}");
        assert!(!json.contains("vector"), "{json}");
        assert!(json.contains("fingerprintShort"), "{json}");
        assert!(!json.contains("\"fingerprint\""), "{json}");
    }
}
