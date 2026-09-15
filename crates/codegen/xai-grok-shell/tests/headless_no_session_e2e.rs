//! `grok agent headless` must complete a turn with no grok.com session.
//!
//! Regression: the grok.com relay is session-authenticated, and headless used
//! to exit 1 with "Headless mode requires a grok.com session" whenever no xAI
//! credential was cached — i.e. every BYOK / custom-endpoint run. Without a
//! session the relay is skipped and the agent is served over stdio instead, so
//! a local ACP client can drive a turn on a bare bearer.
//!
//! The sandbox has no `auth.json` and carries a bearer in `XAI_API_KEY` aimed
//! at the mock server, which is exactly that shape.
//!
//! Requires a binary built from the current tree:
//! `cargo build -p xai-grok-pager-bin --bin xai-grok-pager`.

use std::time::Duration;

use xai_grok_test_support::*;

/// `--no-leader` so the agent runs in this process rather than behind a leader
/// (`agent stdio` alone exits 2); `--always-approve` so a turn cannot stall on
/// a permission request the raw client refuses.
const HEADLESS_ARGS: &[&str] = &["agent", "--no-leader", "--always-approve", "headless"];

const TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
async fn agent_headless_completes_a_turn_without_a_grok_session() {
    let server = MockInferenceServer::start_with_models(vec![
        MockModelEntry::with_agent_type("grok-4.5", "grok-build").with_api_backend("responses"),
    ])
    .await
    .expect("start mock server");
    let workdir = git_workdir();

    let mut agent =
        RawStdioClient::spawn_with_args(&server, workdir.workspace(), HEADLESS_ARGS).await;

    agent
        .send_line(
            &serde_json::json!({
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
                        "clientType": "headless-no-session-e2e",
                        "clientVersion": "1.0",
                    },
                },
            })
            .to_string(),
        )
        .await;
    let init_resp = agent.response_for_id("init-1", "initialize", TIMEOUT).await;
    assert!(
        init_resp.get("result").is_some(),
        "initialize must respond with a result, got: {init_resp}\nstderr:\n{}",
        stderr_tail(&agent.stderr(), 1200)
    );

    // The session-less fallback must announce itself: reaching the relay would
    // mean the run still depends on a grok.com session.
    assert!(
        agent.stderr().contains("serving the agent over stdio"),
        "expected the stdio fallback notice on stderr, got:\n{}",
        stderr_tail(&agent.stderr(), 1200)
    );

    agent
        .send_line(
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": "auth-2",
                "method": "authenticate",
                "params": { "methodId": "xai.api_key", "_meta": { "headless": true } },
            })
            .to_string(),
        )
        .await;
    let auth_resp = agent
        .response_for_id("auth-2", "authenticate", TIMEOUT)
        .await;
    assert!(
        auth_resp.get("error").is_none(),
        "authenticate must accept the env bearer: {auth_resp}\nstderr:\n{}",
        stderr_tail(&agent.stderr(), 1200)
    );

    agent
        .send_line(
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": "new-3",
                "method": "session/new",
                "params": { "cwd": workdir.workspace(), "mcpServers": [] },
            })
            .to_string(),
        )
        .await;
    let new_resp = agent.response_for_id("new-3", "session/new", TIMEOUT).await;
    let session_id = new_resp["result"]["sessionId"]
        .as_str()
        .unwrap_or_else(|| {
            panic!(
                "session/new must return a sessionId, got: {new_resp}\nstderr:\n{}",
                stderr_tail(&agent.stderr(), 1200)
            )
        })
        .to_string();

    agent
        .send_line(
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": "prompt-4",
                "method": "session/prompt",
                "params": {
                    "sessionId": session_id,
                    "prompt": [{ "type": "text", "text": "say hello" }],
                },
            })
            .to_string(),
        )
        .await;
    let prompt_resp = agent
        .response_for_id("prompt-4", "session/prompt", TIMEOUT)
        .await;
    assert!(
        prompt_resp.get("error").is_none(),
        "the turn must complete: {prompt_resp}\nrequest log:\n{}\nstderr:\n{}",
        server.request_log_summary(),
        stderr_tail(&agent.stderr(), 1200)
    );
    assert!(
        prompt_resp["result"]["stopReason"].is_string(),
        "prompt response should carry a stopReason, got: {prompt_resp}"
    );
    assert!(
        server.request_count() > 0,
        "mock server received no inference requests\nstderr:\n{}",
        stderr_tail(&agent.stderr(), 1200)
    );
}
