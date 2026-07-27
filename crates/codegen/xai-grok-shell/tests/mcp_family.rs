//! Consolidated integration-test root for MCP-related actor/model tests.
//!
//! Boundaries:
//! - Tests here exercise rmcp serialization/tool registration and the
//!   permission-manager actor for MCP grant persistence.
//! - They do not require a pre-built `grok` binary, leader socket, or special
//!   host resources.
//! - `mcp_permission_persistence` keeps its `serial_test` annotations; under
//!   `cargo test` they serialize use of the shared-per-binary `GROK_HOME`. Under
//!   nextest each test runs in its own process, so the temp directory is still
//!   isolated.

#[path = "mcp_family/test_mcp_integration.rs"]
mod test_mcp_integration;
#[path = "mcp_family/test_mcp_permission_persistence.rs"]
mod test_mcp_permission_persistence;
