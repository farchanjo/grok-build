//! `GROK_XAI_ENABLED=0` must win over a present grok.com session: the headless
//! entry point keeps the `grok.com` relay websocket closed even when `auth.json`
//! (here: the inline `GROK_AUTH` form) holds an x.ai session.
//!
//! Regression: the relay is picked *before* `bootstrap`, which is where the
//! switch used to be applied — so a dropped hoist in `run_headless_inner` makes
//! a present session outrank the switch and the run opens the relay again.
//!
//! The relay URL points at a closed loopback port, so a connect *attempt* is
//! visible as the `relay_connecting` instrumentation event on stderr
//! (`RUST_LOG=info`) without depending on the network.
//!
//! Requires a binary built from the current tree:
//! `cargo build -p xai-grok-pager-bin --bin xai-grok-pager`.

use std::time::Duration;

use xai_grok_test_support::*;

/// `--no-leader` so the agent runs in this process rather than behind a leader
/// (`agent stdio` alone exits 2); `--always-approve` so a turn cannot stall on
/// a permission request the raw client refuses.
const HEADLESS_ARGS: &[&str] = &["agent", "--no-leader", "--always-approve", "headless"];

/// Printed by the headless entry point when it decides not to build the relay.
const RELAY_OFF_NOTICE: &str = "the grok.com relay is off";
/// Instrumentation event emitted by the relay loop on every connect attempt.
const RELAY_CONNECTING: &str = "relay_connecting";
/// A closed loopback port: the WebSocket handshake fails immediately, so an
/// attempt is unambiguous and needs no network.
const BLACKHOLE_WS_URL: &str = "ws://127.0.0.1:1/ws";

/// Budget for the child to reach the relay decision. Generous: the entry point
/// does auth + config work first, and CI boxes run the suite in parallel.
const WAIT: Duration = Duration::from_secs(30);
/// After the decision is observed, how long to keep watching for the opposite
/// signal (a wrongly-built relay logs its attempt immediately).
const GRACE: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(100);
const TIMEOUT: Duration = Duration::from_secs(30);

/// An x.ai OIDC session, i.e. `GrokAuth::is_xai_auth() == true` — the shape the
/// relay gate accepts. Inline `GROK_AUTH` keeps the sandbox write-free.
fn grok_session_json() -> String {
    serde_json::json!({
        "key": "session-bearer",
        "auth_mode": "oidc",
        "create_time": "2026-01-01T00:00:00Z",
        "user_id": "relay-switch-e2e",
        "oidc_issuer": "https://auth.x.ai",
        "refresh_token": "rt",
        "expires_at": "2099-01-01T00:00:00Z",
    })
    .to_string()
}

fn init_line() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "init-1",
        "method": "initialize",
        "params": {
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": { "readTextFile": false, "writeTextFile": false },
                "terminal": false,
            },
            "_meta": {
                "startupHints": {
                    "nonInteractive": true,
                    "skipGitStatus": true,
                    "skipProjectLayout": true,
                },
                "clientType": "xai-switch-relay-e2e",
                "clientVersion": "1.0",
            },
        },
    })
    .to_string()
}

/// Spawn `agent headless` with a grok.com session and the given switch value.
async fn spawn_headless_with_session(server: &MockInferenceServer, switch: &str) -> RawStdioClient {
    let mut sandbox = git_workdir();
    sandbox.set_env("GROK_AUTH", grok_session_json());
    sandbox.set_env("GROK_XAI_ENABLED", switch);
    sandbox.set_env("GROK_WS_URL", BLACKHOLE_WS_URL);
    // The relay logs its connect attempts through tracing, which is quiet by
    // default on this entrypoint.
    sandbox.set_env("RUST_LOG", "info");
    let cwd = sandbox.workspace().to_path_buf();
    RawStdioClient::spawn_with_args_in_sandbox(server, &cwd, HEADLESS_ARGS, sandbox).await
}

async fn wait_for_stderr(agent: &RawStdioClient, needle: &str, budget: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + budget;
    loop {
        if agent.stderr().contains(needle) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(POLL).await;
    }
}

async fn mock_server() -> MockInferenceServer {
    MockInferenceServer::start_with_models(vec![
        MockModelEntry::with_agent_type("grok-4.5", "grok-build").with_api_backend("responses"),
    ])
    .await
    .expect("start mock server")
}

/// The switch, not the credential, decides: with a session present and
/// `GROK_XAI_ENABLED=0` no websocket connect may be attempted, and the run still
/// serves the agent (over stdio) so the session stays usable.
#[tokio::test]
async fn xai_switch_keeps_the_relay_off_with_a_session_present() {
    let server = mock_server().await;
    let mut agent = spawn_headless_with_session(&server, "0").await;

    assert!(
        wait_for_stderr(&agent, RELAY_OFF_NOTICE, WAIT).await,
        "the headless entry point must announce the relay is off, got:\n{}\n{}",
        stderr_tail(&agent.stderr(), 1200),
        agent.process_diagnostics()
    );
    // Grace period: the relay task (if wrongly built) logs its attempt at once.
    tokio::time::sleep(GRACE).await;
    assert!(
        !agent.stderr().contains(RELAY_CONNECTING),
        "no grok.com websocket may be attempted while the switch is off, got:\n{}\n{}",
        stderr_tail(&agent.stderr(), 1200),
        agent.process_diagnostics()
    );

    // The run is still usable: it serves the agent over stdio, with the session
    // present on disk.
    agent.send_line(&init_line()).await;
    let init_resp = agent.response_for_id("init-1", "initialize", TIMEOUT).await;
    assert!(
        init_resp.get("result").is_some(),
        "initialize must respond with a result, got: {init_resp}\nstderr:\n{}",
        stderr_tail(&agent.stderr(), 1200)
    );
}

/// Control: the same session with the switch on must still reach for the relay.
#[tokio::test]
async fn session_still_relays_when_the_switch_is_on() {
    let server = mock_server().await;
    let mut agent = spawn_headless_with_session(&server, "1").await;

    assert!(
        wait_for_stderr(&agent, RELAY_CONNECTING, WAIT).await,
        "control: a present session must still open the relay, got:\n{}\n{}",
        stderr_tail(&agent.stderr(), 1200),
        agent.process_diagnostics()
    );
    assert!(
        !agent.stderr().contains(RELAY_OFF_NOTICE),
        "the stdio fallback notice must not be printed when the relay is up, got:\n{}\n{}",
        stderr_tail(&agent.stderr(), 1200),
        agent.process_diagnostics()
    );

    // This shape does not read stdin, so it does not exit on EOF; stop it here
    // rather than leaving a relay-retry loop behind the test binary.
    agent.start_kill();
    tokio::time::sleep(Duration::from_millis(300)).await;
}
