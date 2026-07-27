//! Consolidated integration-test root for in-process agent/session runtime tests.
//!
//! Boundaries chosen to reduce high Rust link-test-binary costs while keeping
//! tests coherent and safe under both `cargo test` (shared process) and
//! nextest (each test in its own process):
//!
//! - One process = all tests here link a single integration test binary.
//! - All tests are hermetic: they use `TempDir`, `MockInferenceServer`, or the
//!   in-process ACP/leader helpers; none require a pre-built `grok` binary or
//!   mutate the surrounding environment.
//!
//! Kept standalone (not in this family):
//! - process-global env tests (`team_managed_config`, `config_update_isolation`);
//! - ignored acceptance tests that require a pre-built binary or special host
//!   resources (`built_binary_e2e`, `agent_type_invariant`, `auth_provider_e2e`,
//!   `global_extra_headers_e2e`, `debug_logging`, `vendor_compat`,
//!   `trusted_local_plugin_refresh_e2e`, `leader_death_repro`, `leader_soak`,
//!   `leader_version_skew`, `refusal_stop_reason`, `stop_hook_e2e`,
//!   `subagent_orphan_reconcile`);
//! - perf/resource repros (`session_load_perf`, `git_contention_e2e`,
//!   `heap_profile_monitor`);
//! - MCP actor tests (`mcp_integration`, `mcp_permission_persistence`) because
//!   the `mcp_permission_persistence` actor uses a shared `GROK_HOME` per binary
//!   and serial execution (`serial_test`), which is easier to reason about in
//!   its own test target;
//! - `test_settings_refresh` and `sandbox_requirements_pin` live in the
//!   `config_family` because they exercise configuration resolution rather than
//!   session runtime;
//! - `test_inference_client` and `test_leader_stdio_integration` are kept as
//!   their own standalone targets: the former exercises pre-existing inference
//!   client assumptions that do not need family-wide behavior changes, and the
//!   latter is Unix-only and reports nextest LEAK warnings when linked into a
//!   shared family binary.

mod common;

#[path = "session_runtime_family/test_active_sessions_smoke.rs"]
mod test_active_sessions_smoke;
#[path = "session_runtime_family/test_doom_loop_recovery.rs"]
mod test_doom_loop_recovery;
#[path = "session_runtime_family/test_doomloop_capture.rs"]
mod test_doomloop_capture;
#[path = "session_runtime_family/test_fork_session.rs"]
mod test_fork_session;
#[path = "session_runtime_family/test_registry_churn.rs"]
mod test_registry_churn;
#[path = "session_runtime_family/test_summary_reasoning_effort.rs"]
mod test_summary_reasoning_effort;
#[path = "session_runtime_family/test_xai_session_update.rs"]
mod test_xai_session_update;
#[path = "session_runtime_family/trace_replay.rs"]
mod trace_replay;
