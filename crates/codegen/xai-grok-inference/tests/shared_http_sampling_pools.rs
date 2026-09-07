//! Per-provider sampling pools (Phase 6b / R5): sampling keeps the single
//! process-wide shared client as the default, but routes through the
//! per-provider `ProviderPoolKey` registry (`pool = "sampling"`) when a
//! provider has configured pool tuning or pins HTTP/1.1. These live in their
//! own integration binary (one process under cargo test / nextest) so the
//! environment they pin cannot leak into, or be poisoned by, other tests.

mod support;

use std::collections::HashMap;
use std::sync::Once;
use std::sync::atomic::Ordering;
use std::time::Duration;

use support::{send_one, test_config};
use xai_grok_inference::config::ProviderIdentity;
use xai_grok_inference::{
    InferenceClient, InferenceConfig, ProviderPoolTuning, configure_provider_pool_tuning,
    sampling_pool_names,
};
use xai_grok_test_support::spawn_counting_server;

/// Pin the env these assertions depend on before any client is built, so
/// ambient shell exports (`GROK_SAMPLER_SHARED_CLIENT=0`,
/// `GROK_POOL_MAX_IDLE=0`) cannot flip the expected pooling behavior.
fn pin_env() {
    static PIN: Once = Once::new();
    PIN.call_once(|| {
        // Safety: runs before any test builds a client or reads these vars;
        // racing tests block on the Once, and the crate latches the kill
        // switch and pool knobs only at first client construction.
        unsafe {
            std::env::remove_var("GROK_SAMPLER_SHARED_CLIENT");
            std::env::remove_var("GROK_POOL_MAX_IDLE");
            std::env::remove_var("GROK_POOL_IDLE_TIMEOUT_SECS");
            std::env::remove_var("GROK_CONNECT_TIMEOUT_SECS");
        }
    });
}

fn tuned_config(base_url: &str, identity: ProviderIdentity) -> InferenceConfig {
    let mut cfg = test_config(base_url, "token");
    cfg.provider_identity = identity;
    cfg
}

/// Register per-provider pool tuning for the given provider ids. The keys
/// must match the adapter's provider id string (`adapter.id().as_str()`),
/// which is how the sampler resolves the per-provider sampling pool.
fn register_tuning(entries: &[(&str, ProviderPoolTuning)]) {
    let overrides: HashMap<String, ProviderPoolTuning> = entries
        .iter()
        .map(|(id, tuning)| ((*id).to_string(), *tuning))
        .collect();
    configure_provider_pool_tuning(overrides);
}

/// Two providers with distinct pool keys (and registered tuning) each get
/// their own sampling client/pool: sequential requests to the same host over
/// two distinct clients open two connections instead of sharing one.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_provider_pool_keys_yield_two_clients() {
    pin_env();
    register_tuning(&[
        (
            "openai",
            ProviderPoolTuning {
                max_idle: Some(4),
                ..ProviderPoolTuning::default()
            },
        ),
        (
            "anthropic",
            ProviderPoolTuning {
                max_idle: Some(4),
                ..ProviderPoolTuning::default()
            },
        ),
    ]);

    let (base_url, accepts, _heads) = spawn_counting_server().await;
    let a = InferenceClient::new(tuned_config(&base_url, ProviderIdentity::OpenAi)).unwrap();
    let b = InferenceClient::new(tuned_config(&base_url, ProviderIdentity::Anthropic)).unwrap();

    // Distinct pool keys ⇒ distinct clients ⇒ distinct warm pools.
    let pools = sampling_pool_names();
    assert!(
        pools.iter().any(|p| p == "sampling/openai"),
        "openai must route through its own sampling pool, got {pools:?}"
    );
    assert!(
        pools.iter().any(|p| p == "sampling/anthropic"),
        "anthropic must route through its own sampling pool, got {pools:?}"
    );

    send_one(&a).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    send_one(&b).await;
    // Two sequential requests to one host over two independent pools must
    // open two connections (no cross-client connection reuse).
    assert_eq!(accepts.load(Ordering::SeqCst), 2);
}

/// An unconfigured provider (no adapter pool tuning and no registered tuning)
/// keeps using the single shared client: two distinct clients to the same
/// host reuse one pooled connection, and no `sampling/*` pool is created.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unconfigured_provider_keeps_shared_client() {
    pin_env();

    let (base_url, accepts, _heads) = spawn_counting_server().await;
    // xai and custom identities are not registered for tuning, so both route
    // through the shared client.
    let a = InferenceClient::new(tuned_config(&base_url, ProviderIdentity::Xai)).unwrap();
    let b = InferenceClient::new(
        test_config(&base_url, "token"), // default identity = Custom
    )
    .unwrap();

    send_one(&a).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    send_one(&b).await;
    // No sampling pool was created for either unconfigured provider.
    let pools = sampling_pool_names();
    assert!(
        !pools.iter().any(|p| p == "sampling/xai"),
        "xai must stay on the shared client, got {pools:?}"
    );
    assert!(
        !pools.iter().any(|p| p == "sampling/custom"),
        "custom must stay on the shared client, got {pools:?}"
    );
    // Both share one pooled connection.
    assert_eq!(accepts.load(Ordering::SeqCst), 1);
}

/// `http1_only` pins a provider's sampling pool: an `http1_only` provider
/// gets its own pool (distinct from the shared client), so its connection is
/// not reused by a default (shared-client) provider hitting the same host.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http1_only_pins_its_pool() {
    pin_env();
    register_tuning(&[(
        "zai",
        ProviderPoolTuning {
            http1_only: Some(true),
            ..ProviderPoolTuning::default()
        },
    )]);

    let (base_url, accepts, _heads) = spawn_counting_server().await;
    let z = InferenceClient::new(tuned_config(&base_url, ProviderIdentity::Zai)).unwrap();
    let d = InferenceClient::new(test_config(&base_url, "token")).unwrap();

    let pools = sampling_pool_names();
    assert!(
        pools.iter().any(|p| p == "sampling/zai"),
        "http1_only zai must route through its own pinned sampling pool, got {pools:?}"
    );

    // zai on its own http1_only pool, then a default provider on the shared
    // client: they must not share a connection.
    send_one(&z).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    send_one(&d).await;
    assert_eq!(accepts.load(Ordering::SeqCst), 2);
}
