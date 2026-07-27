//! Consolidated integration-test root for configuration-resolving tests.
//!
//! Boundaries:
//! - Tests here are deterministic, small, and do not mutate the surrounding
//!   environment or require a pre-built `grok` binary.
//! - `sandbox_requirements_pin` validates requirement-pinned sandbox profile
//!   resolution.
//! - `test_settings_refresh` validates the mock `/v1/settings` endpoint and the
//!   blocking settings client round-trip.
//!
//! Process-global env tests (`config_update_isolation`) and the much larger
//! `team_managed_config` suite are kept standalone because they share one
//! `GROK_HOME` per binary via `OnceLock` and `#[serial]`.

#[path = "config_family/sandbox_requirements_pin.rs"]
mod sandbox_requirements_pin;
#[path = "config_family/test_settings_refresh.rs"]
mod test_settings_refresh;
