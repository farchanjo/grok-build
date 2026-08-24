//! Actor-level tests for the `/context` usage categories: populated rows
//! with counts, compat-harness suppression of the MCP row, and parity
//! between the MCP snapshot and the injected reminder.
use super::support::*;
use super::*;
use crate::session::tool_index::{ServerMetadata, ToolMetadata};
fn mcp_tool(server: &str, tool: &str) -> ToolMetadata {
    ToolMetadata {
        qualified_name: format!("{server}__{tool}"),
        server_name: server.to_string(),
        tool_name: tool.to_string(),
        description: format!("{tool} description"),
        parameters: vec!["arg".to_string()],
        input_schema: serde_json::json!({"type": "object"}),
    }
}
fn install_mcp_servers(actor: &SessionActor) {
    let mut snapshot = actor.tool_metadata_snapshot.lock().unwrap();
    snapshot.tools = vec![mcp_tool("demo", "echo"), mcp_tool("demo", "add")];
    snapshot.servers = vec![ServerMetadata {
        name: "demo".to_string(),
        description: Some("A demo server.".to_string()),
    }];
    snapshot.mcp_initialized = true;
}
async fn seed_skills(actor: &SessionActor, names: &[&str]) {
    let skills = names
        .iter()
        .map(
            |name| xai_grok_tools::implementations::skills::types::SkillInfo {
                name: name.to_string(),
                description: format!("Does {name} things."),
                path: format!("/skills/{name}/SKILL.md"),
                ..Default::default()
            },
        )
        .collect();
    let bridge = actor.tool_bridge_handle();
    bridge
        .seed_skill_discovery(None, None, skills, None, None, None, Default::default())
        .await;
}
#[tokio::test(flavor = "current_thread")]
async fn usage_categories_include_skills_and_mcp_with_counts() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            seed_skills(&actor, &["alpha", "beta"]).await;
            install_mcp_servers(&actor);
            let rows = actor.usage_categories().await;
            assert_eq!(rows.len(), 2, "{rows:?}");
            let skills = &rows[0];
            assert_eq!(skills.label, "Skills");
            assert_eq!(skills.detail.as_deref(), Some("2 skills"));
            assert!(skills.tokens > 0);
            let mcp = &rows[1];
            assert_eq!(mcp.label, "MCP servers");
            assert_eq!(mcp.detail.as_deref(), Some("1 server"));
            assert!(mcp.tokens > 0);
            let info = actor.build_session_info().await;
            assert_eq!(info.context.usage_categories.len(), 2);
        })
        .await;
}
/// Anti-drift pin for the MCP row: the estimated snapshot must equal the body
/// `maybe_inject_mcp_reminder` injects in `Full` mode, minus the
/// `<system-reminder>` wrapper. Composing the two texts differently (for
/// example, dropping the tool usage hint from one side) fails this test.
#[tokio::test(flavor = "current_thread")]
async fn mcp_snapshot_matches_full_mode_injected_reminder() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.mcp_reminder_mode = McpReminderMode::Full;
            install_mcp_servers(&actor);
            let snapshot = actor
                .mcp_announcement_snapshot()
                .await
                .expect("servers installed");
            assert_eq!(snapshot.server_count, 1);
            actor
                .mcp_reminder_dirty
                .store(true, std::sync::atomic::Ordering::Relaxed);
            actor.maybe_inject_mcp_reminder().await;
            let conversation = actor.chat_state_handle.get_conversation().await;
            let injected = conversation
                .last()
                .expect("reminder injected")
                .text_content();
            let body = injected
                .strip_prefix("<system-reminder>\n")
                .and_then(|s| s.strip_suffix("\n</system-reminder>"))
                .unwrap_or_else(|| panic!("unexpected wrapper: {injected}"));
            assert_eq!(body, snapshot.text);
        })
        .await;
}

/// An old server payload without the prime field deserializes with `prime:
/// None` (additive only).
#[test]
fn prime_context_info_old_wire_deserializes_without_prime() {
    let c: crate::session::ContextInfo = serde_json::from_str(r#"{"used":1,"total":2}"#).unwrap();
    assert!(c.prime.is_none(), "old wire must deserialize without prime");
}

/// Serializing a prime outcome omits empty arrays, zero injected budgets, and
/// absent status so old clients never see new fields they cannot parse.
#[test]
fn prime_context_info_new_wire_skips_empty_arrays_and_none_status() {
    use crate::session::PrimeContextInfo;

    // Default has no status, no names, no degradation: everything skipped.
    let empty = serde_json::to_string(&PrimeContextInfo::default()).unwrap();
    assert_eq!(
        empty, "{}",
        "empty prime runs must serialize to a bare object"
    );

    // A documented status still skips the empty arrays/counters.
    let p = PrimeContextInfo {
        status: Some("disabled".into()),
        ..Default::default()
    };
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains("\"status\":\"disabled\""), "{json}");
    assert!(!json.contains("degradation"), "{json}");
    assert!(!json.contains("primedSkillNames"), "{json}");
    assert!(!json.contains("recommendedAgentNames"), "{json}");
    assert!(!json.contains("selectionMode"), "{json}");
    assert!(!json.contains("readiness"), "{json}");

    // Round-trips.
    let back: PrimeContextInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.status.as_deref(), Some("disabled"));
    assert!(back.degradation.is_empty());
    assert!(
        !back.should_render(),
        "human session-info suppresses the empty default state while JSON retains it"
    );
}

/// The nested `prime` field round-trips on `ContextInfo` and is skipped when
/// absent.
#[test]
fn context_info_prime_field_round_trips_and_skips_when_absent() {
    use crate::session::PrimeContextInfo;

    let present = crate::session::ContextInfo {
        prime: Some(PrimeContextInfo {
            status: Some("primed".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let v = serde_json::to_value(&present).unwrap();
    assert_eq!(v["prime"]["status"], "primed");
    let round: crate::session::ContextInfo = serde_json::from_value(v).unwrap();
    assert_eq!(round.prime.unwrap().status.as_deref(), Some("primed"));

    let absent = serde_json::to_string(&crate::session::ContextInfo::default()).unwrap();
    assert!(!absent.contains("\"prime\""), "{absent}");
}

/// `build_session_info` projects the actor-local outcome into `context.prime`,
/// and an absent outcome leaves it `None`.
#[tokio::test(flavor = "current_thread")]
async fn build_session_info_includes_last_prime_outcome_when_recorded() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;

            // No prime recorded yet.
            assert!(actor.build_session_info().await.context.prime.is_none());
            // Reload truth: seed an outcome and confirm the snapshot generation
            // used, then confirm a later registry change does not remap it.
            *actor.last_prime_outcome.borrow_mut() = Some(LastPrimeOutcome {
                retrieval_snapshot_generation: Some(7),
                retrieval_profile: Some("my-profile".into()),
                primed_skill_names: vec!["deploy".into()],
                recommended_agent_names: vec!["explore".into()],
                injected_chars: 1200,
                injected_tokens: 600,
                status: PrimeOutcomeStatus::Primed,
                ..LastPrimeOutcome::default()
            });
            let info = actor.build_session_info().await;
            let prime = info.context.prime.expect("prime present after record");
            assert_eq!(prime.retrieval_snapshot_generation, Some(7));
            assert_eq!(prime.status.as_deref(), Some("primed"));
            assert_eq!(prime.primed_skill_names, vec!["deploy".to_string()]);
            assert_eq!(prime.recommended_agent_names, vec!["explore".to_string()]);
        })
        .await;
}
