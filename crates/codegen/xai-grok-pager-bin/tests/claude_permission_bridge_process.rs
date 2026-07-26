//! True process-level MCP permission-bridge integration (PR7 R2).
//!
//! Spawns the freshly built `xai-grok-pager` hidden subcommand
//! `__claude-permission-bridge` with the auth token only in the environment,
//! speaks MCP Content-Length over stdio, and verifies authenticated UDS
//! allow/deny plus unauthorized rejection and parent-death exit.
//!
//! No installed `grok`, no real Claude, no network, no secrets from disk.
//! Requires `--features claude-cli-runtime`.

#![cfg(all(feature = "claude-cli-runtime", unix))]

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use xai_grok_shell::agent::external_runtime::claude_cli::permission_bridge::{
    self, BRIDGE_SOCKET_ENV, BRIDGE_TOKEN_ENV, BRIDGE_TOOL_NAME, ClaudePermissionRequest,
    ClaudePermissionResponse, PERMISSION_BRIDGE_SUBCOMMAND, PermissionBrokerServer, ScriptedBroker,
};

fn bin_path() -> std::path::PathBuf {
    std::env::var_os("CARGO_BIN_EXE_xai-grok-pager")
        .map(std::path::PathBuf::from)
        .expect("CARGO_BIN_EXE_xai-grok-pager must be set by cargo test")
}

fn write_mcp(stdin: &mut impl Write, value: &Value) {
    let body = serde_json::to_vec(value).unwrap();
    write!(stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    stdin.write_all(&body).unwrap();
    stdin.flush().unwrap();
}

fn read_mcp(stdout: &mut impl BufRead) -> Value {
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        let n = stdout.read_line(&mut line).expect("header line");
        assert!(n > 0, "unexpected EOF in MCP headers");
        if line == "\r\n" || line == "\n" {
            break;
        }
        headers.push_str(&line);
    }
    let mut len = None;
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            len = Some(rest.trim().parse::<usize>().unwrap());
        }
    }
    let len = len.expect("Content-Length");
    let mut buf = vec![0u8; len];
    stdout.read_exact(&mut buf).expect("body");
    serde_json::from_slice(&buf).expect("json")
}

fn tool_text(resp: &Value) -> String {
    resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_owned()
}

fn tool_is_error(resp: &Value) -> bool {
    resp["result"]["isError"].as_bool().unwrap_or(true)
}

fn parse_behavior(text: &str) -> ClaudePermissionResponse {
    serde_json::from_str(text).expect("behavior json")
}

/// Full ~/.grokdev isolation for any accidental resource load.
fn grokdev_env(cmd: &mut Command) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let grok = format!("{home}/.grokdev");
    cmd.env("GROK_HOME", &grok);
    cmd.env("GROK_LEADER_SOCKET", format!("{grok}/leader.sock"));
    cmd.env("GROK_DISABLE_AUTOUPDATER", "1");
    for k in [
        "GROK_CURSOR_SKILLS_ENABLED",
        "GROK_CURSOR_RULES_ENABLED",
        "GROK_CURSOR_AGENTS_ENABLED",
        "GROK_CURSOR_MCPS_ENABLED",
        "GROK_CURSOR_HOOKS_ENABLED",
        "GROK_CURSOR_SESSIONS_ENABLED",
        "GROK_CLAUDE_SKILLS_ENABLED",
        "GROK_CLAUDE_RULES_ENABLED",
        "GROK_CLAUDE_AGENTS_ENABLED",
        "GROK_CLAUDE_MCPS_ENABLED",
        "GROK_CLAUDE_HOOKS_ENABLED",
        "GROK_CLAUDE_SESSIONS_ENABLED",
        "GROK_CODEX_SKILLS_ENABLED",
        "GROK_CODEX_RULES_ENABLED",
        "GROK_CODEX_AGENTS_ENABLED",
        "GROK_CODEX_MCPS_ENABLED",
        "GROK_CODEX_HOOKS_ENABLED",
        "GROK_CODEX_SESSIONS_ENABLED",
    ] {
        cmd.env(k, "0");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(claude_cli_env)]
async fn real_bridge_subprocess_mcp_allow_deny_auth_and_parent_death() {
    let broker = Arc::new(ScriptedBroker::new(vec![
        ClaudePermissionResponse::allow(),
        ClaudePermissionResponse::deny("policy deny: blocked"),
    ]));
    let cancel = CancellationToken::new();
    let server = PermissionBrokerServer::start(broker.clone(), cancel.clone())
        .await
        .expect("broker");

    let mut child = Command::new(bin_path());
    child
        .arg(PERMISSION_BRIDGE_SUBCOMMAND)
        .arg("--socket")
        .arg(server.socket_path())
        .env(BRIDGE_SOCKET_ENV, server.socket_path())
        .env(BRIDGE_TOKEN_ENV, server.token())
        // Token must NOT appear in argv.
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    grokdev_env(&mut child);
    let mut child = child.spawn().expect("spawn bridge child");

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);

    // initialize
    write_mcp(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "pr7-test", "version": "0" }
            }
        }),
    );
    let init = read_mcp(&mut reader);
    assert_eq!(init["id"], 1);
    assert!(init.get("result").is_some());

    // tools/list
    write_mcp(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    );
    let list = read_mcp(&mut reader);
    let tools = list["result"]["tools"].as_array().expect("tools");
    assert!(
        tools.iter().any(|t| t["name"] == BRIDGE_TOOL_NAME),
        "permission_prompt tool listed"
    );

    // tools/call → allow
    write_mcp(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": BRIDGE_TOOL_NAME,
                "arguments": {
                    "tool_name": "Read",
                    "tool_use_id": "t1",
                    "input": { "file_path": "a.rs" }
                }
            }
        }),
    );
    let allow_resp = read_mcp(&mut reader);
    assert_eq!(allow_resp["id"], 3);
    assert!(!tool_is_error(&allow_resp), "allow: isError must be false");
    let allow_body = parse_behavior(&tool_text(&allow_resp));
    assert!(matches!(allow_body, ClaudePermissionResponse::Allow { .. }));

    // tools/call → deny (isError: false, behavior: deny)
    write_mcp(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": BRIDGE_TOOL_NAME,
                "arguments": {
                    "tool_name": "Bash",
                    "tool_use_id": "t2",
                    "input": { "command": "echo x" }
                }
            }
        }),
    );
    let deny_resp = read_mcp(&mut reader);
    assert_eq!(deny_resp["id"], 4);
    assert!(!tool_is_error(&deny_resp), "deny: isError must be false");
    let deny_body = parse_behavior(&tool_text(&deny_resp));
    assert!(
        matches!(deny_body, ClaudePermissionResponse::Deny { ref message, .. } if message.contains("deny") || message.contains("blocked")),
        "got {deny_body:?}"
    );

    // Unauthorized UDS client (wrong token) fail closed.
    let bad = permission_bridge::forward_to_broker_for_test(
        server.socket_path(),
        "wrong-token-not-authorized-zzzz",
        ClaudePermissionRequest {
            tool_use_id: Some("bad".into()),
            tool_name: "Read".into(),
            input: json!({}),
            decision_reason: None,
            blocked_path: None,
            agent_id: None,
        },
    )
    .await;
    assert!(
        bad.is_err()
            || matches!(
                bad,
                Ok(ClaudePermissionResponse::Deny { ref message, .. })
                    if message.contains("unauthorized")
            ),
        "unauthorized: {bad:?}"
    );

    // Parent death: shutdown broker → child should EOF / exit.
    server.shutdown().await;
    drop(stdin); // close MCP stdin so child can notice
    let status = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::task::spawn_blocking(move || child.wait())
            .await
            .unwrap()
    })
    .await
    .expect("child should exit after broker shutdown")
    .expect("wait");
    // Exit 0 (clean EOF) or non-zero (disconnect fail closed) — both non-hanging.
    let _ = status;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(claude_cli_env)]
async fn bridge_rejects_missing_token_env() {
    let broker = Arc::new(ScriptedBroker::new(vec![]));
    let cancel = CancellationToken::new();
    let server = PermissionBrokerServer::start(broker, cancel).await.unwrap();

    let mut child = Command::new(bin_path());
    child
        .arg(PERMISSION_BRIDGE_SUBCOMMAND)
        .arg("--socket")
        .arg(server.socket_path())
        .env(BRIDGE_SOCKET_ENV, server.socket_path())
        // Intentionally omit BRIDGE_TOKEN_ENV
        .env_remove(BRIDGE_TOKEN_ENV)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    grokdev_env(&mut child);
    let out = child.output().expect("run");
    assert!(!out.status.success(), "missing token must fail closed");
    server.shutdown().await;
}
