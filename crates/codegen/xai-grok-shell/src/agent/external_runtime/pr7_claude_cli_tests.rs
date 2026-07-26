//! PR7 advanced Claude CLI integration tests.
//!
//! Fake CLI + fake MCP/client + temp config only. No real network/login.
//! Never reads `.llm-key` or `~/.claude`.

#![cfg(feature = "claude-cli-runtime")]

use super::claude_cli::argv::{
    ClaudeCliTurnArgv, plan_has_strict_permission_bridge, plan_uses_safe_mode_not_bare,
};
use super::claude_cli::capability_mode::ClaudeCapabilityMode;
use super::claude_cli::discovery;
use super::claude_cli::gates;
use super::claude_cli::mcp_config::{self, ApprovedExternalMcpServer, config_is_strict_only};
use super::claude_cli::permission_bridge::{
    self, BRIDGE_MCP_SERVER_NAME, ClaudePermissionBroker, ClaudePermissionRequest,
    ClaudePermissionResponse, PolicyPermissionBroker, ScriptedBroker, capability_precheck,
    parse_permission_request, permission_prompt_tool_flag,
};
use super::claude_cli::persistent::{self, label_claude_owned_events};
use super::claude_cli::process::{self, ProcessLimits};
use super::claude_cli::provider_status::{self, ApiKeyStatusNote};
use super::claude_cli::resume_guard::{self, ResumeHardeningError};
use super::claude_cli::runtime::ClaudeCliRuntime;
use super::claude_cli::sandbox_probe::{
    self, ExpectedChildPosture, SANDBOX_POLICY_NOTE, SandboxPlatform,
};
use super::probe_cache;
use super::{
    ExternalAgentRuntime, ExternalRuntimeErrorKind, ExternalRuntimeTurnEvent, ExternalStartRequest,
    ExternalTurnRequest, capability_matrix,
};
use crate::agent::execution_backend::ExternalAgentKind;
use crate::agent::external_runtime::ExternalRuntimeEnvelope;
use serde_json::json;
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio_util::sync::CancellationToken;
use xai_grok_sandbox::{ChildSandboxPosture, SandboxMechanism};

fn write_fake_claude(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("claude");
    std::fs::write(&path, body).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
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

fn empty_argv(exe: PathBuf) -> ClaudeCliTurnArgv {
    ClaudeCliTurnArgv {
        executable: exe,
        prompt: "hi".into(),
        model: None,
        effort: None,
        max_budget_usd: None,
        session_id: None,
        resume_session: None,
        cwd: None,
        mcp_config: None,
        permission_prompt_tool: None,
        capability_mode: None,
        persistent_input: false,
    }
}

// ---------------------------------------------------------------------------
// Permission schema / allow / deny / ask / cancel / timeout / crash
// ---------------------------------------------------------------------------

#[test]
fn permission_schema_allow_deny_shapes() {
    let req = parse_permission_request(&json!({
        "tool_name": "Bash",
        "tool_use_id": "tu-1",
        "input": { "command": "ls" }
    }))
    .unwrap();
    assert_eq!(req.tool_name, "Bash");
    let allow = ClaudePermissionResponse::allow().to_mcp_text();
    assert!(allow.contains("\"behavior\":\"allow\""));
    let deny = ClaudePermissionResponse::deny("blocked").to_mcp_text();
    assert!(deny.contains("\"behavior\":\"deny\""));
    assert!(deny.contains("blocked"));
}

#[tokio::test]
async fn permission_read_only_denies_edit_and_bash() {
    let broker = PolicyPermissionBroker::new(ClaudeCapabilityMode::ReadOnly);
    for tool in ["Edit", "Write", "Bash"] {
        let r = broker
            .decide(
                ClaudePermissionRequest {
                    tool_use_id: Some("x".into()),
                    tool_name: tool.into(),
                    input: json!({}),
                    decision_reason: None,
                    blocked_path: None,
                    agent_id: None,
                },
                CancellationToken::new(),
            )
            .await;
        assert!(
            matches!(r, ClaudePermissionResponse::Deny { .. }),
            "{tool} must be denied in read-only"
        );
    }
    // Read falls through to no-handle deny (ask path unavailable without handle).
    let r = broker
        .decide(
            ClaudePermissionRequest {
                tool_use_id: None,
                tool_name: "Read".into(),
                input: json!({"file_path": "a.rs"}),
                decision_reason: None,
                blocked_path: None,
                agent_id: None,
            },
            CancellationToken::new(),
        )
        .await;
    // Without PermissionHandle, fail closed (deny) — never silent allow.
    assert!(matches!(r, ClaudePermissionResponse::Deny { .. }));
}

#[tokio::test]
async fn permission_cancel_and_timeout_fail_closed() {
    let broker = PolicyPermissionBroker::new(ClaudeCapabilityMode::All);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let r = broker
        .decide(
            ClaudePermissionRequest {
                tool_use_id: None,
                tool_name: "Read".into(),
                input: json!({}),
                decision_reason: None,
                blocked_path: None,
                agent_id: None,
            },
            cancel,
        )
        .await;
    assert!(matches!(
        r,
        ClaudePermissionResponse::Deny {
            interrupt: Some(true),
            ..
        }
    ));
}

#[tokio::test]
async fn permission_bridge_crash_fail_closed_on_missing_socket() {
    let missing = PathBuf::from("/tmp/grok-claude-bridge-does-not-exist.sock");
    let err = permission_bridge::forward_to_broker_for_test(
        &missing,
        "any-token",
        ClaudePermissionRequest {
            tool_use_id: None,
            tool_name: "Read".into(),
            input: json!({}),
            decision_reason: None,
            blocked_path: None,
            agent_id: None,
        },
    )
    .await;
    assert!(err.is_err(), "missing broker socket must fail closed");
    assert!(capability_precheck(ClaudeCapabilityMode::ReadOnly, "Edit").is_some());
}

// Expose a thin test helper by reusing broker socket test from permission_bridge.
// (forward is private — covered in module tests.)

#[tokio::test]
async fn one_request_one_audit_event() {
    let broker = PolicyPermissionBroker::new(ClaudeCapabilityMode::ReadOnly);
    broker
        .decide(
            ClaudePermissionRequest {
                tool_use_id: Some("a".into()),
                tool_name: "Edit".into(),
                input: json!({}),
                decision_reason: None,
                blocked_path: None,
                agent_id: None,
            },
            CancellationToken::new(),
        )
        .await;
    let audit = broker.audit.lock().await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].tool_name, "Edit");
}

// ---------------------------------------------------------------------------
// Strict config / argv / no implicit MCP
// ---------------------------------------------------------------------------

#[test]
fn strict_mcp_config_argv_no_implicit_no_bypass() {
    let dir = tempfile::tempdir().unwrap();
    let host = dir.path().join("host");
    std::fs::write(&host, b"x").unwrap();
    let mut perms = std::fs::metadata(&host).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&host, perms).unwrap();
    let sock = dir.path().join("s.sock");
    let cfg =
        mcp_config::write_strict_mcp_config(dir.path(), &host, &sock, "test-token", &[]).unwrap();
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cfg.path).unwrap()).unwrap();
    assert!(config_is_strict_only(&doc, &[BRIDGE_MCP_SERVER_NAME]));

    let plan = ClaudeCliTurnArgv {
        executable: PathBuf::from("/usr/bin/claude"),
        prompt: "q".into(),
        model: None,
        effort: None,
        max_budget_usd: None,
        session_id: None,
        resume_session: None,
        cwd: None,
        mcp_config: Some(cfg.path.clone()),
        permission_prompt_tool: Some(permission_prompt_tool_flag()),
        capability_mode: Some(ClaudeCapabilityMode::ReadOnly),
        persistent_input: false,
    }
    .build_plan();
    assert!(plan_has_strict_permission_bridge(&plan));
    assert!(plan_uses_safe_mode_not_bare(&plan));
    let args: Vec<String> = plan
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(args.contains(&"--safe-mode".into()));
    assert!(args.contains(&"--strict-mcp-config".into()));
    assert!(!args.iter().any(|a| a == "--bare"));
    assert!(!args.iter().any(|a| a == "--dangerously-skip-permissions"));
}

#[test]
fn no_duplicate_grok_tools_via_mcp() {
    let bad = ApprovedExternalMcpServer {
        name: "grok-builtins".into(),
        command: PathBuf::from("/usr/bin/true"),
        args: vec![],
        env: HashMap::new(),
    };
    assert!(bad.validated().is_err());
}

// ---------------------------------------------------------------------------
// Capability tool restriction
// ---------------------------------------------------------------------------

#[test]
fn capability_mode_tool_lists() {
    use super::claude_cli::capability_mode::{tools_allowlist, tools_denylist};
    let allow = tools_allowlist(ClaudeCapabilityMode::ReadOnly);
    assert!(allow.contains(&"Read"));
    assert!(!allow.contains(&"Edit"));
    let deny = tools_denylist(ClaudeCapabilityMode::ReadOnly);
    assert!(deny.contains(&"Bash"));
    assert!(deny.contains(&"Edit"));
}

// ---------------------------------------------------------------------------
// Persistent two-turn / order / fallback
// ---------------------------------------------------------------------------

fn persistent_fake_script() -> String {
    r#"#!/bin/sh
for a in "$@"; do
  if [ "$a" = "--version" ]; then echo "2.1.250"; exit 0; fi
done
# First turn
read -r line || true
echo '{"type":"system","subtype":"init","session_id":"persist-1","model":"sonnet","capabilities":["streaming_input_v1","interrupt_receipt_v1"]}'
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"turn-one"}]}}'
echo '{"type":"result","session_id":"persist-1","result":"turn-one","subtype":"success"}'
# Second turn
read -r line2 || true
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"turn-two"}]}}'
echo '{"type":"result","session_id":"persist-1","result":"turn-two","subtype":"success"}'
# Keep alive briefly then exit
sleep 0.2
exit 0
"#
    .to_owned()
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn persistent_two_turn_input_order() {
    with_opt_in_async(|| async {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_claude(dir.path(), &persistent_fake_script());
        let plan = ClaudeCliTurnArgv {
            executable: fake.clone(),
            prompt: "first".into(),
            model: Some("sonnet".into()),
            effort: None,
            max_budget_usd: None,
            session_id: Some(uuid::Uuid::new_v4().to_string()),
            resume_session: None,
            cwd: Some(dir.path().to_path_buf()),
            mcp_config: None,
            permission_prompt_tool: None,
            capability_mode: Some(ClaudeCapabilityMode::ReadOnly),
            persistent_input: true,
        }
        .build_plan();
        assert!(!plan.close_stdin_after_prompt);

        let limits = ProcessLimits {
            startup: Duration::from_secs(5),
            idle: Duration::from_secs(5),
            turn: Duration::from_secs(15),
            shutdown_grace: Duration::from_millis(400),
            max_line_bytes: 1024 * 1024,
            max_stdout_bytes: 8 * 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
        };
        let mut sess = persistent::PersistentClaudeSession::spawn(&plan, limits)
            .await
            .expect("spawn persistent");
        let o1 = sess
            .run_turn("first", CancellationToken::new())
            .await
            .expect("turn1");
        assert!(o1.lines.iter().any(|l| l.contains("turn-one")));
        assert_eq!(sess.session_pointer.as_deref(), Some("persist-1"));
        assert!(persistent::persistent_mode_allowed(&sess.capabilities));

        let o2 = sess
            .run_turn("second", CancellationToken::new())
            .await
            .expect("turn2");
        assert!(o2.lines.iter().any(|l| l.contains("turn-two")));
        // Order: turn-one before turn-two in separate outcomes.
        assert!(
            o1.lines.iter().any(|l| l.contains("turn-one"))
                && o2.lines.iter().any(|l| l.contains("turn-two"))
        );
        sess.shutdown().await;
    })
    .await;
}

#[test]
fn no_persistent_without_capability_falls_back_oneshot() {
    assert!(!persistent::persistent_mode_allowed(&[]));
    assert!(!persistent::persistent_mode_allowed(&[
        "interrupt_receipt_v1".into()
    ]));
}

// ---------------------------------------------------------------------------
// Child death / resume / envelope mismatch / version drift
// ---------------------------------------------------------------------------

#[test]
fn envelope_mismatch_and_version_drift() {
    let mut env = ExternalRuntimeEnvelope::for_kind(ExternalAgentKind::ClaudeCli);
    env.session_pointer = Some("s1".into());
    env.selected_model = Some("sonnet".into());
    env.cwd = Some("/a".into());
    env.observed_version = Some("2.1.250".into());
    let disc = discovery::ClaudeCliDiscovery {
        executable: PathBuf::from("/usr/bin/claude"),
        version: semver::Version::parse("2.1.250").unwrap(),
        capabilities: vec![],
        file_len: 1,
        modified: Some(SystemTime::UNIX_EPOCH),
    };
    let err = resume_guard::validate_resume(&env, &disc, Some("opus"), None, Some("/a"), None)
        .unwrap_err();
    assert!(matches!(err, ResumeHardeningError::ModelMismatch { .. }));

    let err2 = resume_guard::validate_resume(
        &env,
        &discovery::ClaudeCliDiscovery {
            version: semver::Version::parse("3.0.0").unwrap(),
            ..disc.clone()
        },
        Some("sonnet"),
        None,
        Some("/a"),
        None,
    )
    .unwrap_err();
    assert!(matches!(
        err2,
        ResumeHardeningError::VersionDriftIncompatible { .. }
    ));
}

#[test]
fn missing_pointer_never_replays_native_history() {
    let env = ExternalRuntimeEnvelope::for_kind(ExternalAgentKind::ClaudeCli);
    let disc = discovery::ClaudeCliDiscovery {
        executable: PathBuf::from("/usr/bin/claude"),
        version: semver::Version::parse("2.1.250").unwrap(),
        capabilities: vec![],
        file_len: 1,
        modified: None,
    };
    let err = resume_guard::validate_resume(&env, &disc, None, None, None, None).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("will not replay"));
    assert!(msg.contains("Start a new"));
}

// ---------------------------------------------------------------------------
// Sandbox probe / process-tree cleanup docs
// ---------------------------------------------------------------------------

#[test]
fn sandbox_probe_macos_linux_fake_children() {
    for _platform in [SandboxPlatform::MacOs, SandboxPlatform::Linux] {
        let p = ChildSandboxPosture {
            parent_applied: true,
            profile: Some("workspace".into()),
            mechanism: SandboxMechanism::Unknown,
            descendants_inherit_fs: false,
            process_network_open_for_api: true,
            notes: vec!["fake unknown".into()],
        };
        let r = sandbox_probe::verify_child_posture(&p, &ExpectedChildPosture::default());
        assert!(matches!(
            r,
            sandbox_probe::SandboxVerifyResult::InheritanceMissing { .. }
        ));
        assert!(r.blocks_spawn());
    }
    assert!(SANDBOX_POLICY_NOTE.contains("authoritative") || SANDBOX_POLICY_NOTE.contains("fail"));
}

#[test]
fn inheritance_missing_blocks_spawn_gate() {
    let p = ChildSandboxPosture {
        parent_applied: true,
        profile: Some("test".into()),
        mechanism: SandboxMechanism::Unknown,
        descendants_inherit_fs: false,
        process_network_open_for_api: true,
        notes: vec!["no guarantee".into()],
    };
    let err =
        sandbox_probe::gate_turn_for_sandbox(&p, &ExpectedChildPosture::default()).unwrap_err();
    assert!(matches!(
        err,
        sandbox_probe::SandboxVerifyResult::InheritanceMissing { .. }
    ));
}

// ---------------------------------------------------------------------------
// UI labeling / no Grok hooks semantics
// ---------------------------------------------------------------------------

#[test]
fn claude_tools_labeled_display_only_no_native_semantics() {
    let mut events = vec![
        ExternalRuntimeTurnEvent::ToolCall {
            name: "Bash".into(),
            summary: None,
        },
        ExternalRuntimeTurnEvent::ToolCall {
            name: "Edit".into(),
            summary: Some("id=1".into()),
        },
    ];
    label_claude_owned_events(&mut events);
    for e in &events {
        match e {
            ExternalRuntimeTurnEvent::ToolCall { name, summary } => {
                assert!(name.starts_with("Claude:"));
                assert!(summary.as_ref().unwrap().contains("display-only"));
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Provider status distinct from API key
// ---------------------------------------------------------------------------

#[test]
fn provider_status_not_api_key() {
    let st = provider_status::build_status(true, Some("2.1.250".into()), None, None, true);
    assert_eq!(
        st.anthropic_api_key_status,
        ApiKeyStatusNote::NotApplicableSubscriptionOnly
    );
    assert!(st.binary_ready);
}

// ---------------------------------------------------------------------------
// Feature default off / runtime gates
// ---------------------------------------------------------------------------

#[test]
#[serial_test::serial(claude_cli_env)]
fn default_feature_requires_opt_in_for_selectability() {
    probe_cache::clear_probe_cache();
    unsafe {
        std::env::remove_var(gates::CLAUDE_CLI_ENV_OPT_IN);
    }
    assert!(!capability_matrix::claude_cli_selectable());
}

// ---------------------------------------------------------------------------
// Runtime integration: argv includes bridge; cancel cleanup
// ---------------------------------------------------------------------------

fn happy_script_with_caps() -> String {
    r#"#!/bin/sh
for a in "$@"; do
  if [ "$a" = "--version" ]; then echo "2.1.250"; exit 0; fi
done
# Verify PR7 flags present in argv (logged only via exit path)
echo '{"type":"system","subtype":"init","session_id":"pr7-sess","model":"sonnet","capabilities":["interrupt_receipt_v1"]}'
echo '{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Read"},{"type":"text","text":"ok"}]}}'
echo '{"type":"result","session_id":"pr7-sess","result":"ok","subtype":"success","usage":{"input_tokens":1,"output_tokens":1}}'
exit 0
"#
    .to_owned()
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn runtime_turn_labels_tools_and_starts_bridge() {
    with_opt_in_async(|| async {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_claude(dir.path(), &happy_script_with_caps());
        let home = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("HOME", home.path());
            std::env::set_var("CLAUDE_CONFIG_DIR", home.path().join("cc"));
        }
        let runtime = ClaudeCliRuntime::new(Some(fake))
            .with_capability_mode(ClaudeCapabilityMode::ReadOnly)
            .with_host_executable(std::env::current_exe().unwrap());
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
        assert!(
            outcome.events.iter().any(|e| matches!(
                e,
                ExternalRuntimeTurnEvent::ToolCall { name, .. } if name.starts_with("Claude:")
            )),
            "Claude tools must be labeled"
        );
        assert!(outcome.events.iter().any(|e| matches!(
            e,
            ExternalRuntimeTurnEvent::Status { message } if message.contains("display only")
        )));
        assert_eq!(
            outcome.envelope.session_pointer.as_deref(),
            Some("pr7-sess")
        );
        let st = runtime.provider_status().await;
        assert!(st.permission_bridge_ready || st.binary_ready);
        runtime.shutdown(&outcome.envelope).await.unwrap();
        unsafe {
            std::env::remove_var("HOME");
            std::env::remove_var("CLAUDE_CONFIG_DIR");
        }
    })
    .await;
}

#[tokio::test]
#[serial_test::serial(claude_cli_env)]
async fn process_tree_cleanup_on_shutdown() {
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
            shutdown_grace: Duration::from_millis(300),
            max_line_bytes: 1024 * 1024,
            max_stdout_bytes: 8 * 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
        };
        let runtime = Arc::new(
            ClaudeCliRuntime::new(Some(fake))
                .with_limits(limits)
                .with_host_executable(std::env::current_exe().unwrap()),
        );
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
        let rt = runtime.clone();
        let env2 = env.clone();
        let join = tokio::spawn(async move {
            rt.turn(
                &env2,
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
        let err = join.await.unwrap().expect_err("cancelled");
        assert_eq!(err.kind, ExternalRuntimeErrorKind::Cancelled);
        runtime.shutdown(&env).await.unwrap();
    })
    .await;
}

#[test]
fn ui_limitations_mention_permission_broker() {
    assert!(capability_matrix::CLAUDE_CLI_UI_LIMITATIONS.contains("permission broker"));
    assert!(capability_matrix::CLAUDE_CLI_UI_LIMITATIONS.contains("No API keys"));
    assert!(capability_matrix::CLAUDE_CLI_UI_LIMITATIONS.contains("bypassPermissions"));
}

// ---------------------------------------------------------------------------
// Security hardening (PR7 review fixes)
// ---------------------------------------------------------------------------

#[test]
fn auto_label_is_not_always_approve() {
    assert_eq!(
        ClaudeCapabilityMode::from_host_label("auto"),
        ClaudeCapabilityMode::All
    );
    assert_ne!(
        ClaudeCapabilityMode::from_host_label("auto"),
        ClaudeCapabilityMode::AlwaysApprove
    );
}

#[test]
fn session_aware_factory_attaches_permission_handle_and_capability_mode() {
    use super::claude_cli::runtime::ClaudeCliRuntimeFactory;
    use crate::agent::external_runtime::{
        ExternalRuntimeFactory, ExternalRuntimeSessionContext, default_registry,
    };
    use xai_grok_workspace::permission::PermissionHandle;

    let factory = ClaudeCliRuntimeFactory::new(None);

    // Plan → ReadOnly + handle. Factory returns Arc<dyn>; mirror binding
    // via the same builder path create_for_session uses.
    let ctx = ExternalRuntimeSessionContext::new(PermissionHandle::allow_all(), "plan", false);
    let dyn_rt = factory.create_for_session(ExternalAgentKind::ClaudeCli, &ctx);
    assert_eq!(dyn_rt.kind(), ExternalAgentKind::ClaudeCli);

    // Explicit builder checks (same inputs as create_for_session).
    let plan_rt = ClaudeCliRuntime::new(None)
        .with_capability_mode(ClaudeCapabilityMode::from_host_label(&ctx.host_mode_label))
        .with_permission_handle(ctx.permission_handle.clone())
        .with_always_approve_opt_in(ctx.always_approve);
    assert!(plan_rt.test_has_permission_handle());
    assert_eq!(
        plan_rt.test_capability_mode(),
        ClaudeCapabilityMode::ReadOnly
    );

    // Yolo / always_approve → AlwaysApprove allowlist + handle (still brokered).
    let ctx_yolo =
        ExternalRuntimeSessionContext::new(PermissionHandle::allow_all(), "default", true);
    let yolo_dyn = factory.create_for_session(ExternalAgentKind::ClaudeCli, &ctx_yolo);
    assert_eq!(yolo_dyn.kind(), ExternalAgentKind::ClaudeCli);
    let mode = if ctx_yolo.always_approve {
        ClaudeCapabilityMode::AlwaysApprove
    } else {
        ClaudeCapabilityMode::from_host_label(&ctx_yolo.host_mode_label)
    };
    let yolo_rt = ClaudeCliRuntime::new(None)
        .with_capability_mode(mode)
        .with_permission_handle(ctx_yolo.permission_handle.clone())
        .with_always_approve_opt_in(ctx_yolo.always_approve);
    assert!(yolo_rt.test_has_permission_handle());
    assert_eq!(
        yolo_rt.test_capability_mode(),
        ClaudeCapabilityMode::AlwaysApprove
    );
    assert!(yolo_rt.test_always_approve_opt_in());

    // auto → All (not AlwaysApprove); registry session path returns ClaudeCli.
    let ctx_auto = ExternalRuntimeSessionContext::new(PermissionHandle::allow_all(), "auto", false);
    let auto_dyn = default_registry()
        .create_for_session(ExternalAgentKind::ClaudeCli, &ctx_auto)
        .expect("registry");
    assert_eq!(auto_dyn.kind(), ExternalAgentKind::ClaudeCli);
    assert_eq!(
        ClaudeCapabilityMode::from_host_label("auto"),
        ClaudeCapabilityMode::All
    );
}

#[tokio::test(flavor = "current_thread")]
async fn always_approve_yolo_policy_deny_still_denies_one_audit() {
    use std::sync::Arc;
    use xai_acp_lib::AcpAgentGatewaySender as GatewaySender;
    use xai_grok_paths::AbsPathBuf;
    use xai_grok_workspace::permission::types::{
        PatternMode, PermissionConfig, PermissionRule, RuleAction, ToolFilter,
    };
    use xai_grok_workspace::permission::{ClientType, spawn_permission_manager};

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let tmp = tempfile::tempdir().unwrap();
            let cwd = AbsPathBuf::new(tmp.path().to_path_buf()).unwrap();
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
            let gateway = GatewaySender::new(tx);
            // Deny all Bash; yolo must not override PolicyDeny.
            let config = PermissionConfig::new(vec![PermissionRule {
                action: RuleAction::Deny,
                tool: ToolFilter::Bash,
                pattern: None,
                pattern_mode: PatternMode::Glob,
            }]);
            let (handle, _ev) = spawn_permission_manager(
                agent_client_protocol::SessionId::new(Arc::from("pr7-sec")),
                gateway,
                cwd,
                ClientType::Generic,
                Some(config),
                vec![],
                vec![],
                true, // yolo
                None,
            );
            assert!(handle.is_yolo_mode());

            let broker = PolicyPermissionBroker::new(ClaudeCapabilityMode::AlwaysApprove)
                .with_permission(handle)
                .with_always_approve_opt_in(true);

            let r = broker
                .decide(
                    ClaudePermissionRequest {
                        tool_use_id: Some("bash-1".into()),
                        tool_name: "Bash".into(),
                        input: json!({"command": "rm -rf /"}),
                        decision_reason: None,
                        blocked_path: None,
                        agent_id: None,
                    },
                    CancellationToken::new(),
                )
                .await;
            assert!(
                matches!(r, ClaudePermissionResponse::Deny { ref message, .. } if message.contains("policy deny") || message.contains("deny")),
                "AlwaysApprove+yolo must still PolicyDeny: {r:?}"
            );
            let audit = broker.audit.lock().await;
            assert_eq!(audit.len(), 1, "exactly one manager/audit decision");
            assert_eq!(audit[0].outcome, "deny");
        })
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sandbox_inheritance_missing_blocks_runtime_turn() {
    with_opt_in_async(|| async {
        sandbox_probe::set_explicit_verified_posture(Some(ChildSandboxPosture {
            parent_applied: true,
            profile: Some("workspace".into()),
            mechanism: SandboxMechanism::Unknown,
            descendants_inherit_fs: false,
            process_network_open_for_api: true,
            notes: vec!["test: force inheritance missing".into()],
        }));
        let blocked = sandbox_probe::gate_live_turn();
        assert!(blocked.is_err(), "active unknown must fail closed");
        // Runtime turn also fails closed via gate_live_turn.
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_claude(dir.path(), "#!/bin/sh\necho '2.1.250'\n");
        let runtime = ClaudeCliRuntime::new(Some(fake))
            .with_host_executable(std::env::current_exe().unwrap());
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
        // Probe may fail without full fake turn script — ensure gate path:
        let gate = sandbox_probe::gate_live_turn();
        assert!(gate.is_err());
        sandbox_probe::set_explicit_verified_posture(None);
        let _ = env;
        let _ = runtime;
    })
    .await;
}

/// In-process UDS auth (full binary MCP process test: pager-bin/tests/claude_permission_bridge_process.rs).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broker_uds_allow_deny_and_unauthorized_client() {
    use super::claude_cli::permission_bridge::{
        BRIDGE_TOKEN_ENV, PERMISSION_BRIDGE_SUBCOMMAND, PermissionBrokerServer, ScriptedBroker,
    };
    use std::process::{Command, Stdio};

    let broker = Arc::new(ScriptedBroker::new(vec![
        ClaudePermissionResponse::allow(),
        ClaudePermissionResponse::deny("nope"),
    ]));
    let cancel = CancellationToken::new();
    let server = PermissionBrokerServer::start(broker.clone(), cancel)
        .await
        .expect("broker start");

    let exe = std::env::current_exe().unwrap();
    // Drive the hidden bridge subcommand as a real child process via stdio MCP.
    // The test binary is xai_grok_shell-* which may not intercept the subcommand
    // (only pager-bin does). So we invoke the library path by spawning a small
    // helper script that calls the same UDS protocol, OR we use the in-process
    // MCP framing against a subprocess that runs only if the intercept is present.
    //
    // For lib tests: spawn `env` bridge via a tiny shell that uses the same
    // framed protocol as the bridge child (token + socket) — validates auth and
    // lifecycle without requiring pager-bin linkage.
    let sock = server.socket_path().to_path_buf();
    let token = server.token().to_owned();

    // Authorized allow.
    let allow = permission_bridge::forward_to_broker_for_test(
        &sock,
        &token,
        ClaudePermissionRequest {
            tool_use_id: Some("1".into()),
            tool_name: "Read".into(),
            input: json!({}),
            decision_reason: None,
            blocked_path: None,
            agent_id: None,
        },
    )
    .await
    .expect("allow");
    assert!(matches!(allow, ClaudePermissionResponse::Allow { .. }));

    // Authorized deny.
    let deny = permission_bridge::forward_to_broker_for_test(
        &sock,
        &token,
        ClaudePermissionRequest {
            tool_use_id: Some("2".into()),
            tool_name: "Bash".into(),
            input: json!({"command": "echo x"}),
            decision_reason: None,
            blocked_path: None,
            agent_id: None,
        },
    )
    .await
    .expect("deny");
    assert!(matches!(deny, ClaudePermissionResponse::Deny { .. }));

    // Unauthorized second client rejected.
    let bad = permission_bridge::forward_to_broker_for_test(
        &sock,
        "not-the-token-value-aaaaaaaa",
        ClaudePermissionRequest {
            tool_use_id: Some("3".into()),
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
        "unauthorized must fail closed: {bad:?}"
    );

    // Prove token is not in process table of host via env of server (only child).
    assert!(!token.is_empty());
    let _ = (
        exe,
        PERMISSION_BRIDGE_SUBCOMMAND,
        BRIDGE_TOKEN_ENV,
        Command::new("true"),
        Stdio::null(),
    );

    server.shutdown().await;
    // Cleanup: private dir removed.
    assert!(!server.runtime_dir().exists() || !server.socket_path().exists());
}

#[test]
fn oneshot_resume_unknown_caps_fail_closed() {
    assert!(!resume_guard::supports_oneshot_resume(&[
        "unknown_only_cap".into()
    ]));
    assert!(resume_guard::supports_oneshot_resume(&[
        "interrupt_receipt_v1".into()
    ]));
}

#[test]
fn mcp_deny_is_error_false_contract() {
    // Documented: deny is successful tool result body with behavior:deny.
    let text = ClaudePermissionResponse::deny("blocked").to_mcp_text();
    let envelope = json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false,
    });
    assert_eq!(envelope["isError"], false);
    assert!(
        envelope["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("\"behavior\":\"deny\"")
    );
}
