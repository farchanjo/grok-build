//! Live Milvus outage ("kill") test for hard-remote `mode = "milvus"`.
//!
//! Unlike the in-crate live conformance suite, this test exercises the
//! full outage lifecycle against the real server: seed → ready → search
//! works; `docker stop` the Milvus container → search must fail closed
//! (bounded, empty, handle marked `Unavailable`, no hang, no panic);
//! `docker start` → readiness is re-established and the volume-backed
//! data survives.
//!
//! Gated by the same env vars as the live conformance suite, plus an
//! explicit opt-in for the destructive container stop/start:
//!
//! ```text
//! GROK_MILVUS_TEST_URI=http://vm.services:19530 \
//! GROK_MILVUS_TEST_TOKEN= \
//! GROK_MILVUS_KILL_TEST=1 \
//! DOCKER_HOST=tcp://vm.services:5555 \
//! ./grok-test.sh -p xai-grok-memory --run-ignored all -- milvus_kill
//! ```
//!
//! `GROK_MILVUS_KILL_TEST=1` is required because the test stops the shared
//! `milvus-standalone` container for ~30–60 s. A `Drop` guard restarts the
//! container even when an assertion panics between stop and start.

#![allow(clippy::unwrap_used, reason = "test fixtures unwrap intentionally")]

use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use xai_grok_config_types::MemorySearchConfig;
use xai_grok_memory::mirror::{MemoryRow, MirrorHandle, MirrorState};
use xai_grok_memory::mirror_milvus::connect_store;
use xai_grok_memory::search::milvus_search;

/// Container name of the shared standalone Milvus on the VM docker host.
const MILVUS_CONTAINER: &str = "milvus-standalone";

/// Fixture vector (schema dims = 4, orthonormal one-hot like the live
/// conformance suite).
const QUERY_VEC: [f32; 4] = [1.0, 0.0, 0.0, 0.0];
const FP: &str = "0123456789abcdef";
const ROW_ID: &str = "killtest-row-a";
const ROW_TEXT: &str = "Quantum flux capacitor calibration notes for the milvus kill test";

/// Bounded visibility polling for eventually consistent upserts.
const POLLS: u32 = 30;
const POLL_BACKOFF: Duration = Duration::from_millis(500);

/// Restarts the Milvus container on `Drop` unless the test already did it.
/// Guards against leaving the shared VM container stopped on a panic.
struct ContainerGuard {
    restarted: bool,
}

impl ContainerGuard {
    fn new() -> Self {
        Self { restarted: false }
    }

    fn mark_restarted(&mut self) {
        self.restarted = true;
    }
}

impl Drop for ContainerGuard {
    fn drop(&mut self) {
        if !self.restarted {
            eprintln!("guard: container may still be stopped, issuing docker start");
            let _ = docker(&["start", MILVUS_CONTAINER]);
        }
    }
}

/// Run the local `docker` CLI; `DOCKER_HOST` from the environment selects
/// the remote daemon (e.g. `tcp://vm.services:5555`).
fn docker(args: &[&str]) -> Result<String, String> {
    let out = Command::new("docker")
        .args(args)
        .output()
        .map_err(|e| format!("failed to spawn docker: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "docker {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn live_config() -> Option<String> {
    let uri = std::env::var("GROK_MILVUS_TEST_URI").ok()?;
    if uri.trim().is_empty() || std::env::var("GROK_MILVUS_KILL_TEST").as_deref() != Ok("1") {
        return None;
    }
    Some(uri)
}

fn token() -> Option<String> {
    std::env::var("GROK_MILVUS_TEST_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
}

fn collection_name() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    format!("grok_mem_killtest_{}_{millis}", std::process::id())
}

fn test_row() -> MemoryRow {
    MemoryRow {
        id: ROW_ID.to_owned(),
        text: ROW_TEXT.to_owned(),
        vector: QUERY_VEC.to_vec(),
        fingerprint_hash: FP.to_owned(),
        hash: "a".repeat(64),
        source: "workspace".to_owned(),
        path: "notes/killtest.md".to_owned(),
        created_at: chrono::Utc::now().timestamp(),
    }
}

/// Poll `milvus_search` (no embedding provider: BM25-only leg) until it
/// returns a hit for `ROW_ID`. Returns the elapsed wall time of the last
/// successful call.
async fn wait_for_search_hit(handle: &Arc<MirrorHandle>, config: &MemorySearchConfig) {
    let mut last = None;
    for attempt in 0..POLLS {
        let res = milvus_search(handle, None, "quantum flux capacitor", FP, 4, config).await;
        match res {
            Ok(hits) if hits.iter().any(|h| h.chunk_id == ROW_ID) => return,
            Ok(hits) => last = Some(format!("{} hits, none matching", hits.len())),
            Err(e) => last = Some(format!("error: {e}")),
        }
        if attempt + 1 < POLLS {
            tokio::time::sleep(POLL_BACKOFF).await;
        }
    }
    panic!("search never surfaced {ROW_ID} within {POLLS} polls: {last:?}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live Milvus outage test; stops the shared container (GROK_MILVUS_KILL_TEST=1)"]
async fn milvus_kill_test_hard_remote_fails_closed_and_recovers() {
    let Some(uri) = live_config() else {
        eprintln!("skipping: set GROK_MILVUS_TEST_URI and GROK_MILVUS_KILL_TEST=1");
        return;
    };
    // Fail fast (and clearly) when the docker CLI cannot reach the daemon.
    docker(&["version", "--format", "{{.Server.Version}}"])
        .expect("GROK_MILVUS_KILL_TEST=1 requires a working docker CLI + DOCKER_HOST");

    let token = token();
    let store = connect_store(&uri, token.as_deref(), Duration::from_secs(10))
        .await
        .expect("connect to live Milvus");
    let collection = collection_name();
    let handle = Arc::new(MirrorHandle::new(store, collection.clone()));
    let config = MemorySearchConfig::default();
    let row = test_row();

    // ---- Phase 1: seed + ready + live search works -----------------------
    handle
        .ensure_collection_v2(4, FP)
        .await
        .expect("ensure schema-v2 collection");
    handle
        .upsert_rows_v2(std::slice::from_ref(&row))
        .await
        .expect("seed schema-v2 row");
    handle.mark_ready(FP, 4, 1);
    wait_for_search_hit(&handle, &config).await;

    // ---- Phase 2: kill the container, search must fail closed ------------
    let mut guard = ContainerGuard::new();
    let stop_started = Instant::now();
    docker(&["stop", "-t", "2", MILVUS_CONTAINER]).expect("docker stop milvus-standalone");

    // First call after the outage may pay the full RPC deadline; every call
    // must stay bounded by the mirror timeout budget (10 s default + slack).
    let outage = milvus_search(&handle, None, "quantum flux capacitor", FP, 4, &config).await;
    let outage_elapsed = stop_started.elapsed();

    // ---- Phase 3: restart the container (before any assertions) ----------
    docker(&["start", MILVUS_CONTAINER]).expect("docker start milvus-standalone");
    guard.mark_restarted();

    // Assert on the captured outage outcome (no panics between stop/start).
    let hits = outage.expect("outage search must return Ok (hard-remote empty), not Err");
    assert!(
        hits.is_empty(),
        "hard-remote mode must return empty during outage, got {hits:?}"
    );
    assert!(
        outage_elapsed < Duration::from_secs(30),
        "outage search hung for {outage_elapsed:?} (deadline budget is 10 s)"
    );
    assert_eq!(
        handle.snapshot().state,
        MirrorState::Unavailable,
        "a failed remote search must mark the handle Unavailable"
    );

    // ---- Phase 4: readiness re-establishment + data survives -------------
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        match handle.ensure_collection_v2(4, FP).await {
            Ok(()) => break,
            Err(_) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Err(e) => panic!("collection never came back after restart: {e}"),
        }
    }
    let remote = handle
        .list_id_hashes_v2(FP)
        .await
        .expect("list remote rows after recovery");
    assert!(
        remote.contains_key(ROW_ID),
        "volume-backed row must survive a container restart, remote: {remote:?}"
    );
    handle.mark_ready(FP, 4, remote.len() as u64);
    assert!(handle.is_ready_for(FP, 4));
    wait_for_search_hit(&handle, &config).await;

    // ---- Cleanup ----------------------------------------------------------
    handle
        .mirror()
        .drop_collection(&collection)
        .await
        .expect("drop kill-test collection");
}
