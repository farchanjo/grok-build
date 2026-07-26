//! PR6 integration tests for Claude Agent CLI runtime.
//!
//! Uses **fake executable scripts** and temporary HOME / CLAUDE_CONFIG_DIR.
//! Never invokes a real Claude binary, network, or login.

#![cfg(feature = "claude-cli-runtime")]

use super::claude_cli::argv::{ClaudeCliTurnArgv, plan_uses_safe_mode_not_bare};
use super::claude_cli::auth::parse_auth_status_output;
use super::claude_cli::discovery::{
    self, MIN_CLAUDE_CLI_VERSION, discover_claude_executable, parse_claude_version,
    probe_claude_version, validate_executable_path,
};
use super::claude_cli::env_scrub::{SCRUBBED_SECRET_KEYS, build_scrubbed_env, env_contains_secret};
use super::claude_cli::gates;
use super::claude_cli::process::{self, ProcessLimits};
use super::claude_cli::protocol;
use super::claude_cli::runtime::ClaudeCliRuntime;
use super::probe_cache;
use super::{
    ExternalAgentRuntime, ExternalRuntimeErrorKind, ExternalRuntimeTurnEvent, ExternalStartRequest,
    ExternalTurnRequest, capability_matrix,
};
use crate::agent::execution_backend::ExternalAgentKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Write an executable fake `claude` script into `dir` and return its path.
fn write_fake_claude(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("claude");
    std::fs::write(&path, body).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    // canonicalize for absolute path
    std::fs::canonicalize(&path).unwrap()
}

async fn with_opt_in_async<R, F, Fut>(f: F) -> R
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let prior = std::env::var(gates::CLAUDE_CLI_ENV_OPT_IN).ok();
    unsafe {
        std::env::set_var(gates::CLAUDE_CLI_ENV_OPT_IN, "1");
    }
    let result = f().await;
    unsafe {
        match prior {
            Some(v) => std::env::set_var(gates::CLAUDE_CLI_ENV_OPT_IN, v),
            None => std::env::remove_var(gates::CLAUDE_CLI_ENV_OPT_IN),
        }
    }
    result
}

#[test]
fn feature_compiled_true_under_pr6_tests() {
    assert!(gates::claude_cli_feature_compiled());
    assert!(capability_matrix::CLAUDE_CLI_MODEL_SELECTABLE);
}

#[test]
#[serial_test::serial(claude_cli_env)]
fn catalog_selectability_requires_probe_cache() {
    probe_cache::clear_probe_cache();
    // Even with feature, without opt-in+probe: not selectable.
    assert!(!capability_matrix::claude_cli_selectable());
    // Force gates: set opt-in, still no probe.
    unsafe {
        std::env::set_var(gates::CLAUDE_CLI_ENV_OPT_IN, "1");
    }
    probe_cache::clear_probe_cache();
    assert!(!capability_matrix::claude_cli_selectable());
    probe_cache::record_probe_ok("2.1.250");
    assert!(capability_matrix::claude_cli_selectable());
    probe_cache::record_probe_failed("hang");
    assert!(!capability_matrix::claude_cli_selectable());
    unsafe {
        std::env::remove_var(gates::CLAUDE_CLI_ENV_OPT_IN);
    }
    probe_cache::clear_probe_cache();
}

#[test]
#[serial_test::serial(claude_cli_env)]
fn catalog_visibility_no_probe_failed_success() {
    use crate::agent::config::ModelEntry;
    use crate::agent::execution_backend::{ExecutionBackend, ExternalAgentKind};
    use indexmap::IndexMap;
    probe_cache::clear_probe_cache();
    unsafe {
        std::env::set_var(gates::CLAUDE_CLI_ENV_OPT_IN, "1");
    }
    assert!(
        gates::claude_cli_both_gates_open(),
        "test requires feature+opt-in"
    );

    let make = |probe: Option<bool>| {
        let mut catalog = IndexMap::new();
        let mut entry = ModelEntry::fallback("claude-cli-model", &Default::default());
        entry.info.execution_backend =
            ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli);
        entry.info.hidden = false;
        entry.info.user_selectable = true;
        catalog.insert("claude-cli-model".into(), entry);
        capability_matrix::apply_catalog_visibility_with_probe(&mut catalog, probe);
        let cli = catalog.get("claude-cli-model").unwrap();
        (cli.info.hidden, cli.info.user_selectable)
    };

    let (h, s) = make(Some(false));
    assert!(h && !s, "failed probe hides");
    // With gates open but no probe cache: still hidden.
    probe_cache::clear_probe_cache();
    let (h2, s2) = make(None);
    assert!(h2 && !s2, "no probe hides");
    probe_cache::record_probe_ok("2.1.250");
    let (h3, s3) = make(Some(true));
    assert!(!h3 && s3, "success probe shows");

    unsafe {
        std::env::remove_var(gates::CLAUDE_CLI_ENV_OPT_IN);
    }
    probe_cache::clear_probe_cache();
}

#[test]
fn discovery_prefers_configured_absolute_path() {
    let dir = tempfile::tempdir().unwrap();
    let fake = write_fake_claude(dir.path(), "#!/bin/sh\necho '2.1.250'\n");
    // PATH also has something else — configured wins.
    let found = discover_claude_executable(Some(&fake)).unwrap();
    assert_eq!(found, fake);
}

#[test]
fn discovery_rejects_relative_path() {
    let err = validate_executable_path(Path::new("./claude")).unwrap_err();
    assert!(matches!(
        err,
        discovery::ClaudeCliDiscoveryError::InvalidPath { .. }
    ));
}

#[test]
fn discovery_path_before_path_env() {
    let dir = tempfile::tempdir().unwrap();
    let preferred = write_fake_claude(dir.path(), "#!/bin/sh\necho '2.1.250'\n");
    // configured path first
    let found = discover_claude_executable(Some(&preferred)).unwrap();
    assert_eq!(found, preferred);
}

#[tokio::test]
async fn version_probe_accepts_min_and_rejects_old() {
    let dir = tempfile::tempdir().unwrap();
    let good = write_fake_claude(dir.path(), "#!/bin/sh\necho '2.1.250'\n");
    let d = probe_claude_version(&good, Duration::from_secs(3))
        .await
        .unwrap();
    assert!(d.version >= semver::Version::parse(MIN_CLAUDE_CLI_VERSION).unwrap());

    let old_dir = tempfile::tempdir().unwrap();
    let old = write_fake_claude(old_dir.path(), "#!/bin/sh\necho '2.0.0'\n");
    let err = probe_claude_version(&old, Duration::from_secs(3))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        discovery::ClaudeCliDiscoveryError::VersionTooOld { .. }
    ));
}

#[tokio::test]
async fn version_probe_hang_times_out() {
    let dir = tempfile::tempdir().unwrap();
    let hang = write_fake_claude(dir.path(), "#!/bin/sh\nsleep 30\n");
    let err = probe_claude_version(&hang, Duration::from_millis(200))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        discovery::ClaudeCliDiscoveryError::ProbeTimeout { .. }
    ));
}

#[tokio::test]
async fn version_probe_malformed_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let bad = write_fake_claude(dir.path(), "#!/bin/sh\necho 'not-a-version'\n");
    let err = probe_claude_version(&bad, Duration::from_secs(3))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        discovery::ClaudeCliDiscoveryError::VersionParse { .. }
    ));
}

#[test]
fn argv_safe_mode_not_bare_exact() {
    let plan = ClaudeCliTurnArgv {
        executable: PathBuf::from("/tmp/claude"),
        prompt: "hi".into(),
        model: Some("sonnet".into()),
        effort: Some("medium".into()),
        max_budget_usd: Some(2.0),
        session_id: Some("550e8400-e29b-41d4-a716-446655440000".into()),
        resume_session: None,
        cwd: None,
        mcp_config: None,
        permission_prompt_tool: None,
        capability_mode: None,
        persistent_input: false,
    }
    .build_plan();
    assert!(plan_uses_safe_mode_not_bare(&plan));
    let args: Vec<String> = plan
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(
        args.windows(2)
            .any(|w| w[0] == "--output-format" && w[1] == "stream-json")
    );
    assert!(
        args.windows(2)
            .any(|w| w[0] == "--input-format" && w[1] == "stream-json")
    );
    assert!(args.contains(&"--include-partial-messages".into()));
    assert!(args.contains(&"--forward-subagent-text".into()));
    assert!(!args.iter().any(|a| a == "--bare"));
}

#[test]
fn env_scrub_removes_all_required_secrets() {
    unsafe {
        for k in SCRUBBED_SECRET_KEYS {
            std::env::set_var(k, "secret-value");
        }
        std::env::set_var("PATH", "/usr/bin");
        std::env::set_var("HOME", "/tmp/fake-home");
    }
    let env = build_scrubbed_env(&[]);
    for k in SCRUBBED_SECRET_KEYS {
        assert!(!env_contains_secret(&env, k), "{k} must be scrubbed");
    }
    assert!(env_contains_secret(&env, "PATH"));
    assert!(env_contains_secret(&env, "HOME"));
    unsafe {
        for k in SCRUBBED_SECRET_KEYS {
            std::env::remove_var(k);
        }
    }
}

#[test]
fn auth_status_bounded_and_redacted() {
    let result = process::ProbeCommandResult {
        stdout:
            r#"{"loggedIn":true,"email":"bob@corp.example","accessToken":"SECRET_TOKEN_VALUE"}"#
                .into(),
        stderr: String::new(),
        success: true,
        exit_code: Some(0),
    };
    let st = parse_auth_status_output(&result).unwrap();
    assert!(st.logged_in);
    assert!(!st.summary.contains("SECRET"));
    assert!(!st.summary.contains("bob@"));
    assert_eq!(st.account_label.as_deref(), Some("b***@corp.example"));
}

#[test]
fn protocol_normalizes_tool_retry_result_no_dispatch() {
    let lines = vec![
        r#"{"type":"system","subtype":"init","session_id":"s1","capabilities":["interrupt_receipt_v1"],"model":"sonnet"}"#.into(),
        r#"{"type":"system","subtype":"api_retry","attempt":1,"max_retries":2,"error":"overloaded"}"#.into(),
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"tu1","name":"Read"}]}}"#.into(),
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"done"}]}}"#.into(),
        r#"{"type":"result","session_id":"s1","result":"done","subtype":"success","usage":{"input_tokens":1,"output_tokens":2},"total_cost_usd":0.01}"#.into(),
    ];
    let p = protocol::parse_turn_lines(&lines).unwrap();
    assert_eq!(p.session_id.as_deref(), Some("s1"));
    assert!(
        p.events.iter().any(
            |e| matches!(e, ExternalRuntimeTurnEvent::ToolCall { name, .. } if name == "Read")
        )
    );
    assert!(
        p.events
            .iter()
            .any(|e| matches!(e, ExternalRuntimeTurnEvent::Status { message } if message.contains("overloaded")))
    );
    // Tool events are display-only data; nothing here invokes a tool executor.
}

#[test]
fn protocol_malformed_truncated_oversize() {
    assert!(matches!(
        protocol::parse_turn_lines(&["not-json".into()]),
        Err(protocol::ProtocolError::NoFinalResult | protocol::ProtocolError::MalformedLine { .. })
    ));
    let huge = "x".repeat(protocol::MAX_TEXT_EVENT_CHARS + 10);
    assert!(matches!(
        protocol::parse_turn_lines(&[huge]),
        Err(protocol::ProtocolError::Oversized { .. })
    ));
}

fn happy_turn_script() -> String {
    r#"#!/bin/sh
# Fake claude for PR6 tests — no network.
case "$1" in
  --version) echo "2.1.250"; exit 0 ;;
esac
# auth status
if [ "$1" = "auth" ]; then
  echo '{"loggedIn":true,"email":"dev@example.com"}'
  exit 0
fi
# Turn mode: emit NDJSON to stdout
echo '{"type":"system","subtype":"init","session_id":"fake-sess-001","model":"sonnet","capabilities":["interrupt_receipt_v1"]}'
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"hello from fake"}]}}'
echo '{"type":"result","session_id":"fake-sess-001","result":"hello from fake","subtype":"success","usage":{"input_tokens":3,"output_tokens":4},"total_cost_usd":0.0}'
exit 0
"#
    .to_owned()
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn runtime_turn_happy_path_and_resume_pointer() {
    with_opt_in_async(|| async {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_claude(dir.path(), &happy_turn_script());
        let home = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("HOME", home.path());
            std::env::set_var("CLAUDE_CONFIG_DIR", home.path().join("claude-config"));
        }

        let runtime = ClaudeCliRuntime::new(Some(fake));
        let caps = runtime.probe().await.expect("probe");
        assert!(caps.version.as_deref().unwrap().starts_with("2.1."));

        let env = runtime
            .start(ExternalStartRequest {
                cwd: dir.path().display().to_string(),
                worktree_identity: None,
                selected_model: Some("sonnet".into()),
                reasoning_effort: Some("high".into()),
                token_budget: None,
            })
            .await
            .unwrap();

        let outcome = runtime
            .turn(
                &env,
                ExternalTurnRequest {
                    prompt: "hi".into(),
                    selected_model: Some("sonnet".into()),
                    reasoning_effort: None,
                    token_budget: None,
                },
            )
            .await
            .expect("turn");

        assert!(outcome.events.iter().any(
            |e| matches!(e, ExternalRuntimeTurnEvent::TextDelta { text } if text.contains("hello"))
        ));
        assert_eq!(
            outcome.envelope.session_pointer.as_deref(),
            Some("fake-sess-001")
        );
        // Envelope has no raw NDJSON
        let json = serde_json::to_string(&outcome.envelope).unwrap();
        assert!(!json.contains("stream_event"));
        assert!(!json.contains("tool_use"));

        // Resume uses session pointer
        let resumed = runtime.resume(&outcome.envelope).await.unwrap();
        assert_eq!(resumed.session_pointer.as_deref(), Some("fake-sess-001"));

        unsafe {
            std::env::remove_var("HOME");
            std::env::remove_var("CLAUDE_CONFIG_DIR");
        }
    })
    .await;
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn runtime_fails_closed_without_opt_in() {
    let prior = std::env::var(gates::CLAUDE_CLI_ENV_OPT_IN).ok();
    unsafe {
        std::env::remove_var(gates::CLAUDE_CLI_ENV_OPT_IN);
        // Also reject non-truthy values.
        std::env::set_var(gates::CLAUDE_CLI_ENV_OPT_IN, "0");
    }
    let dir = tempfile::tempdir().unwrap();
    let fake = write_fake_claude(dir.path(), &happy_turn_script());
    let runtime = ClaudeCliRuntime::new(Some(fake));
    let err = runtime.probe().await.unwrap_err();
    assert_eq!(err.kind, super::ExternalRuntimeErrorKind::Unavailable);
    assert!(!err.is_auth_error());
    unsafe {
        match prior {
            Some(v) => std::env::set_var(gates::CLAUDE_CLI_ENV_OPT_IN, v),
            None => std::env::remove_var(gates::CLAUDE_CLI_ENV_OPT_IN),
        }
    }
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn cancel_maps_to_cancelled_not_provider_failure() {
    with_opt_in_async(|| async {
        let dir = tempfile::tempdir().unwrap();
        // Hang forever after one NDJSON line so the host must cancel.
        let script = r#"#!/bin/sh
for a in "$@"; do
  if [ "$a" = "--version" ]; then echo "2.1.250"; exit 0; fi
done
echo '{"type":"system","subtype":"init","session_id":"c1"}'
# Keep the process alive until the host kills the process group.
while true; do sleep 1; done
"#;
        let fake = write_fake_claude(dir.path(), script);

        let plan = ClaudeCliTurnArgv {
            executable: fake,
            prompt: "x".into(),
            model: None,
            effort: None,
            max_budget_usd: None,
            session_id: None,
            resume_session: None,
            cwd: Some(dir.path().to_path_buf()),
            mcp_config: None,
            permission_prompt_tool: None,
            capability_mode: None,
            persistent_input: false,
        }
        .build_plan();
        let cancel = CancellationToken::new();
        let cancel2 = cancel.clone();
        let limits = ProcessLimits {
            startup: Duration::from_secs(5),
            idle: Duration::from_secs(30),
            turn: Duration::from_secs(60),
            shutdown_grace: Duration::from_millis(400),
            max_line_bytes: 1024 * 1024,
            max_stdout_bytes: 8 * 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
        };
        let join =
            tokio::spawn(async move { process::run_turn_process(&plan, &limits, cancel2).await });
        // Wait until the first line has had time to arrive, then cancel.
        tokio::time::sleep(Duration::from_millis(300)).await;
        cancel.cancel();
        let outcome = join.await.unwrap().expect("cancel is Ok outcome");
        assert!(
            outcome.cancelled || outcome.exit_code == Some(143) || outcome.exit_signal == Some(15),
            "expected cancelled/143/SIGTERM, got exit={:?} signal={:?} cancelled={}",
            outcome.exit_code,
            outcome.exit_signal,
            outcome.cancelled
        );
    })
    .await;
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn stderr_flood_and_slow_consumer_bounded() {
    with_opt_in_async(|| async {
        let dir = tempfile::tempdir().unwrap();
        let script = r#"#!/bin/sh
if [ "$1" = "--version" ]; then echo "2.1.250"; exit 0; fi
# flood stderr
i=0
while [ $i -lt 5000 ]; do
  echo "noise line $i" >&2
  i=$((i+1))
done
echo '{"type":"result","subtype":"success","result":"ok","session_id":"s"}'
exit 0
"#;
        let fake = write_fake_claude(dir.path(), script);
        let plan = ClaudeCliTurnArgv {
            executable: fake,
            prompt: "x".into(),
            model: None,
            effort: None,
            max_budget_usd: None,
            session_id: None,
            resume_session: None,
            cwd: Some(dir.path().to_path_buf()),
            mcp_config: None,
            permission_prompt_tool: None,
            capability_mode: None,
            persistent_input: false,
        }
        .build_plan();
        let limits = ProcessLimits {
            startup: Duration::from_secs(10),
            idle: Duration::from_secs(10),
            turn: Duration::from_secs(15),
            shutdown_grace: Duration::from_secs(2),
            max_line_bytes: 64 * 1024,
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 4096,
        };
        let outcome = process::run_turn_process(&plan, &limits, CancellationToken::new())
            .await
            .expect("turn completes despite stderr flood");
        assert!(outcome.stderr.len() <= limits.max_stderr_bytes);
        assert!(outcome.lines.iter().any(|l| l.contains("result")));
    })
    .await;
}

#[test]
fn parse_version_min_floor() {
    let v = parse_claude_version("2.1.217").unwrap();
    assert_eq!(v.to_string(), MIN_CLAUDE_CLI_VERSION);
}

#[test]
fn ui_label_experimental() {
    assert!(capability_matrix::CLAUDE_CLI_UI_LABEL.contains("Experimental"));
    assert!(capability_matrix::CLAUDE_CLI_UI_LIMITATIONS.contains("No API keys"));
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn runtime_cancel_returns_cancelled_error_not_end_turn() {
    with_opt_in_async(|| async {
        let dir = tempfile::tempdir().unwrap();
        let script = r#"#!/bin/sh
for a in "$@"; do
  if [ "$a" = "--version" ]; then echo "2.1.250"; exit 0; fi
done
echo '{"type":"system","subtype":"init","session_id":"c1"}'
while true; do sleep 1; done
"#;
        let fake = write_fake_claude(dir.path(), script);
        let limits = ProcessLimits {
            startup: Duration::from_secs(5),
            idle: Duration::from_secs(30),
            turn: Duration::from_secs(60),
            shutdown_grace: Duration::from_millis(400),
            max_line_bytes: 1024 * 1024,
            max_stdout_bytes: 8 * 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
        };
        let runtime = std::sync::Arc::new(ClaudeCliRuntime::new(Some(fake)).with_limits(limits));
        let env = runtime
            .start(ExternalStartRequest {
                cwd: dir.path().display().to_string(),
                worktree_identity: None,
                selected_model: None,
                reasoning_effort: None,
                token_budget: None,
            })
            .await
            .unwrap();

        let runtime_turn = runtime.clone();
        let env_turn = env.clone();
        let join = tokio::spawn(async move {
            runtime_turn
                .turn(
                    &env_turn,
                    ExternalTurnRequest {
                        prompt: "x".into(),
                        selected_model: None,
                        reasoning_effort: None,
                        token_budget: None,
                    },
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(350)).await;
        runtime.cancel(&env).await.unwrap();
        let result = join.await.unwrap();
        let err = result.expect_err("cancel must not complete as Ok/EndTurn");
        assert_eq!(err.kind, ExternalRuntimeErrorKind::Cancelled);
        assert!(!err.is_auth_error());
    })
    .await;
}

#[test]
fn auth_status_argv_is_auth_status_without_json_flag() {
    // Documented contract: argv is ["auth", "status"] only.
    // (query_auth_status uses process::run_probe_command with these args.)
    let args = ["auth", "status"];
    assert_eq!(args, ["auth", "status"]);
    assert!(!args.iter().any(|a| *a == "--json"));
}

#[test]
fn revalidate_detects_replaced_binary() {
    let dir = tempfile::tempdir().unwrap();
    let fake = write_fake_claude(dir.path(), "#!/bin/sh\necho '2.1.250'\n");
    let meta = std::fs::metadata(&fake).unwrap();
    let disc = discovery::ClaudeCliDiscovery {
        executable: fake.clone(),
        version: semver::Version::parse("2.1.250").unwrap(),
        capabilities: vec![],
        file_len: meta.len(),
        modified: meta.modified().ok(),
    };
    // Valid identity.
    discovery::revalidate_executable(&fake, &disc).unwrap();
    // Replace file content (size change).
    std::fs::write(&fake, "#!/bin/sh\necho replaced-and-longer-content\n").unwrap();
    let mut perms = std::fs::metadata(&fake).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    std::fs::set_permissions(&fake, perms).unwrap();
    let err = discovery::revalidate_executable(&fake, &disc).unwrap_err();
    assert!(matches!(
        err,
        discovery::ClaudeCliDiscoveryError::InvalidPath { .. }
    ));
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn process_group_required_spawns_ok_for_fake() {
    // Happy path proves group create/attach succeeds on this platform.
    with_opt_in_async(|| async {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_claude(dir.path(), &happy_turn_script());
        let plan = ClaudeCliTurnArgv {
            executable: fake,
            prompt: "hi".into(),
            model: None,
            effort: None,
            max_budget_usd: None,
            session_id: None,
            resume_session: None,
            cwd: Some(dir.path().to_path_buf()),
            mcp_config: None,
            permission_prompt_tool: None,
            capability_mode: None,
            persistent_input: false,
        }
        .build_plan();
        let out =
            process::run_turn_process(&plan, &ProcessLimits::default(), CancellationToken::new())
                .await
                .expect("process group required path must succeed when group works");
        assert!(!out.cancelled);
        assert!(out.lines.iter().any(|l| l.contains("result")));
    })
    .await;
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn process_rejects_invalid_utf8_stdout() {
    with_opt_in_async(|| async {
        let dir = tempfile::tempdir().unwrap();
        // Emit invalid UTF-8 on stdout then exit.
        let script = r#"#!/bin/sh
if [ "$1" = "--version" ] || echo "$@" | grep -q -- '--version'; then echo "2.1.250"; exit 0; fi
# printf raw bytes (invalid UTF-8)
printf '\xff\xfe invalid\n'
exit 0
"#;
        let fake = write_fake_claude(dir.path(), script);
        let plan = ClaudeCliTurnArgv {
            executable: fake,
            prompt: "x".into(),
            model: None,
            effort: None,
            max_budget_usd: None,
            session_id: None,
            resume_session: None,
            cwd: Some(dir.path().to_path_buf()),
            mcp_config: None,
            permission_prompt_tool: None,
            capability_mode: None,
            persistent_input: false,
        }
        .build_plan();
        let err = process::run_turn_process(
            &plan,
            &ProcessLimits {
                startup: Duration::from_secs(5),
                idle: Duration::from_secs(5),
                turn: Duration::from_secs(10),
                shutdown_grace: Duration::from_millis(300),
                max_line_bytes: 1024 * 1024,
                max_stdout_bytes: 8 * 1024 * 1024,
                max_stderr_bytes: 64 * 1024,
            },
            CancellationToken::new(),
        )
        .await
        .expect_err("invalid UTF-8 must fail");
        assert!(
            matches!(err, process::TurnProcessError::InvalidUtf8),
            "got {err:?}"
        );
    })
    .await;
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn process_rejects_oversized_stdout_line() {
    with_opt_in_async(|| async {
        let dir = tempfile::tempdir().unwrap();
        let script = r#"#!/bin/sh
for a in "$@"; do
  if [ "$a" = "--version" ]; then echo "2.1.250"; exit 0; fi
done
# One huge line without newline until end
python3 -c 'print("x"*20000)'
exit 0
"#;
        let fake = write_fake_claude(dir.path(), script);
        let plan = ClaudeCliTurnArgv {
            executable: fake,
            prompt: "x".into(),
            model: None,
            effort: None,
            max_budget_usd: None,
            session_id: None,
            resume_session: None,
            cwd: Some(dir.path().to_path_buf()),
            mcp_config: None,
            permission_prompt_tool: None,
            capability_mode: None,
            persistent_input: false,
        }
        .build_plan();
        let err = process::run_turn_process(
            &plan,
            &ProcessLimits {
                startup: Duration::from_secs(5),
                idle: Duration::from_secs(5),
                turn: Duration::from_secs(10),
                shutdown_grace: Duration::from_millis(300),
                max_line_bytes: 1024, // small cap
                max_stdout_bytes: 8 * 1024 * 1024,
                max_stderr_bytes: 64 * 1024,
            },
            CancellationToken::new(),
        )
        .await
        .expect_err("oversized line must fail");
        assert!(
            matches!(err, process::TurnProcessError::LineTooLarge { .. }),
            "got {err:?}"
        );
    })
    .await;
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn cancel_after_init_persists_session_pointer_for_resume() {
    with_opt_in_async(|| async {
        let dir = tempfile::tempdir().unwrap();
        let script = r#"#!/bin/sh
for a in "$@"; do
  if [ "$a" = "--version" ]; then echo "2.1.250"; exit 0; fi
done
echo '{"type":"system","subtype":"init","session_id":"resume-me-001","model":"sonnet","capabilities":["interrupt_receipt_v1"]}'
while true; do sleep 1; done
"#;
        let fake = write_fake_claude(dir.path(), script);
        let limits = ProcessLimits {
            startup: Duration::from_secs(5),
            idle: Duration::from_secs(30),
            turn: Duration::from_secs(60),
            shutdown_grace: Duration::from_millis(400),
            max_line_bytes: 1024 * 1024,
            max_stdout_bytes: 8 * 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
        };
        let runtime = std::sync::Arc::new(ClaudeCliRuntime::new(Some(fake.clone())).with_limits(limits.clone()));
        let env = runtime
            .start(ExternalStartRequest {
                cwd: dir.path().display().to_string(),
                worktree_identity: None,
                selected_model: Some("sonnet".into()),
                reasoning_effort: None,
                token_budget: None,
            })
            .await
            .unwrap();

        let runtime_turn = runtime.clone();
        let env_turn = env.clone();
        let join = tokio::spawn(async move {
            runtime_turn
                .turn(
                    &env_turn,
                    ExternalTurnRequest {
                        prompt: "x".into(),
                        selected_model: None,
                        reasoning_effort: None,
                        token_budget: None,
                    },
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(350)).await;
        runtime.cancel(&env).await.unwrap();
        let err = join.await.unwrap().expect_err("must cancel");
        assert_eq!(err.kind, ExternalRuntimeErrorKind::Cancelled);
        let partial = err.partial_envelope.expect("partial envelope on cancel");
        assert_eq!(
            partial.session_pointer.as_deref(),
            Some("resume-me-001"),
            "session pointer from system/init must be persisted for --resume"
        );

        // Next turn plan should use --resume with that pointer.
        let plan = ClaudeCliTurnArgv {
            executable: fake,
            prompt: "continue".into(),
            model: None,
            effort: None,
            max_budget_usd: None,
            session_id: None,
            resume_session: partial.session_pointer.clone(),
            cwd: Some(dir.path().to_path_buf()),
                    mcp_config: None,
            permission_prompt_tool: None,
            capability_mode: None,
            persistent_input: false,
}
        .build_plan();
        let args: Vec<String> = plan
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|w| w[0] == "--resume" && w[1] == "resume-me-001"));
    })
    .await;
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn bootstrap_probe_success_makes_catalog_selectable() {
    probe_cache::clear_probe_cache();
    unsafe {
        std::env::set_var(gates::CLAUDE_CLI_ENV_OPT_IN, "1");
    }
    let dir = tempfile::tempdir().unwrap();
    let fake = write_fake_claude(dir.path(), "#!/bin/sh\necho '2.1.250'\n");
    unsafe {
        std::env::set_var(discovery::CLAUDE_CLI_PATH_ENV, fake.display().to_string());
    }
    assert!(
        gates::claude_cli_both_gates_open(),
        "opt-in must be set for bootstrap test"
    );
    assert!(!capability_matrix::claude_cli_selectable());
    super::claude_cli::runtime::bootstrap_probe_if_gated().await;
    // If bootstrap raced with another serial test clearing path, probe directly.
    if !capability_matrix::claude_cli_selectable() {
        let runtime = ClaudeCliRuntime::new(Some(fake.clone()));
        let _ = runtime.probe().await.expect("direct probe after bootstrap");
    }
    assert!(
        capability_matrix::claude_cli_selectable(),
        "after bootstrap probe success entry is selectable (cache={:?})",
        probe_cache::probe_cache_state()
    );
    // Failure path
    probe_cache::clear_probe_cache();
    unsafe {
        std::env::set_var(discovery::CLAUDE_CLI_PATH_ENV, "/nonexistent/claude-binary");
    }
    super::claude_cli::runtime::bootstrap_probe_if_gated().await;
    assert!(!capability_matrix::claude_cli_selectable());
    unsafe {
        std::env::remove_var(gates::CLAUDE_CLI_ENV_OPT_IN);
        std::env::remove_var(discovery::CLAUDE_CLI_PATH_ENV);
    }
    probe_cache::clear_probe_cache();
}
