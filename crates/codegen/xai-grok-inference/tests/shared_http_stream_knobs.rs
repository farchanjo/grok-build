//! HTTP/2 multiplex knob tests in their own integration binary: a separate
//! test binary is a separate process under cargo test, nextest, and Bazel
//! alike, so the env writes below cannot poison other tests and land before
//! the crate's once-per-process client latch first resolves.

mod support;

use std::sync::atomic::Ordering;
use std::time::Duration;

use futures_util::StreamExt;

use support::{conversation_request, test_config};
use xai_grok_inference::InferenceClient;
use xai_grok_test_support::spawn_counting_server;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn h2_multiplex_knobs_client_builds_and_streams_against_mock() {
    // Safety: the only test in this binary, set before any client exists; no
    // concurrent env reads are possible. These knobs are read only by the
    // client builders under test (`GROK_HTTP2_ADAPTIVE_WINDOW` truthy and a
    // positive `GROK_HTTP2_INITIAL_STREAM_WINDOW_SIZE`).
    unsafe {
        std::env::set_var("GROK_HTTP2_ADAPTIVE_WINDOW", "1");
        std::env::set_var("GROK_HTTP2_INITIAL_STREAM_WINDOW_SIZE", "1048576");
    }
    let (base_url, accepts, _heads) = spawn_counting_server().await;
    let client = InferenceClient::new(test_config(&base_url, "token-a")).unwrap();

    let (mut stream, _meta) = client
        .conversation_stream(conversation_request())
        .await
        .expect("knob-bearing client must build and open the stream");

    // The counting server answers `application/json {}` (not SSE): the SSE
    // parser folds it as an unknown field, so the stream ends with no item.
    // The assertions below pin that the stream opens and terminates instead
    // of hanging, and that exactly one request reaches the server (the h2
    // knobs must not perturb the wire behavior).
    tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("stream must terminate, not hang, with the HTTP/2 knobs enabled");
    assert_eq!(accepts.load(Ordering::SeqCst), 1);
}
