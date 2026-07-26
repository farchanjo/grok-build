//! Fail-closed unavailable external runtime (PR5).
//!
//! Never spawns a process. Every operation returns a deterministic non-auth
//! `EXTERNAL_RUNTIME_UNAVAILABLE` error until PR6 provides a real implementation.

use super::{
    ExternalAgentRuntime, ExternalRuntimeCapabilities, ExternalRuntimeEnvelope,
    ExternalRuntimeError, ExternalRuntimeStatus, ExternalStartRequest, ExternalTurnOutcome,
    ExternalTurnRequest,
};
use crate::agent::execution_backend::ExternalAgentKind;
use async_trait::async_trait;

/// Stub runtime for kinds that are typed but not yet implemented in this build.
pub struct UnavailableExternalRuntime {
    kind: ExternalAgentKind,
}

impl UnavailableExternalRuntime {
    pub fn new(kind: ExternalAgentKind) -> Self {
        Self { kind }
    }

    fn err(&self) -> ExternalRuntimeError {
        ExternalRuntimeError::unavailable(self.kind)
    }
}

#[async_trait]
impl ExternalAgentRuntime for UnavailableExternalRuntime {
    fn kind(&self) -> ExternalAgentKind {
        self.kind
    }

    async fn probe(&self) -> Result<ExternalRuntimeCapabilities, ExternalRuntimeError> {
        Err(self.err())
    }

    async fn start(
        &self,
        _request: ExternalStartRequest,
    ) -> Result<ExternalRuntimeEnvelope, ExternalRuntimeError> {
        Err(self.err())
    }

    async fn resume(
        &self,
        _envelope: &ExternalRuntimeEnvelope,
    ) -> Result<ExternalRuntimeEnvelope, ExternalRuntimeError> {
        Err(self.err())
    }

    async fn turn(
        &self,
        _envelope: &ExternalRuntimeEnvelope,
        _request: ExternalTurnRequest,
    ) -> Result<ExternalTurnOutcome, ExternalRuntimeError> {
        Err(self.err())
    }

    async fn cancel(
        &self,
        _envelope: &ExternalRuntimeEnvelope,
    ) -> Result<(), ExternalRuntimeError> {
        Err(self.err())
    }

    async fn shutdown(
        &self,
        _envelope: &ExternalRuntimeEnvelope,
    ) -> Result<(), ExternalRuntimeError> {
        // Shutdown is best-effort; still report unavailable so callers know
        // there is no live process to tear down.
        Err(self.err())
    }

    fn status(&self, _envelope: Option<&ExternalRuntimeEnvelope>) -> ExternalRuntimeStatus {
        ExternalRuntimeStatus::Unavailable
    }
}
