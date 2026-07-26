//! Factory / registry for external agent runtimes.

use super::{ExternalAgentRuntime, UnavailableExternalRuntime};
use crate::agent::execution_backend::ExternalAgentKind;
use std::collections::HashMap;
use std::sync::Arc;

/// Session-bound inputs for constructing an external runtime.
///
/// Carries the live [`PermissionHandle`] and the **single effective** capability
/// mode (after plan-over-yolo precedence) so production Claude CLI attaches real
/// policy without downcasting the runtime trait object.
#[derive(Clone)]
pub struct ExternalRuntimeSessionContext {
    /// Live session permission manager (PolicyDeny wins under yolo).
    pub permission_handle: xai_grok_workspace::permission::PermissionHandle,
    /// Final effective mode key after plan > yolo precedence
    /// (`read_only`, `always_approve`, `all`, …). Used for both runtime
    /// configuration and retained-runtime compatibility.
    pub effective_mode: String,
}

impl ExternalRuntimeSessionContext {
    pub fn new(
        permission_handle: xai_grok_workspace::permission::PermissionHandle,
        effective_mode: impl Into<String>,
    ) -> Self {
        Self {
            permission_handle,
            effective_mode: effective_mode.into(),
        }
    }
}

/// Creates an [`ExternalAgentRuntime`] for a given kind.
pub trait ExternalRuntimeFactory: Send + Sync {
    /// Create a runtime without session binding (probes / bootstrap only).
    fn create(&self, kind: ExternalAgentKind) -> Arc<dyn ExternalAgentRuntime>;

    /// Create a session-bound runtime with live permission + capability mode.
    ///
    /// Default ignores session context (stubs). Production factories override
    /// to attach [`PermissionHandle`] and derive capability mode.
    fn create_for_session(
        &self,
        kind: ExternalAgentKind,
        ctx: &ExternalRuntimeSessionContext,
    ) -> Arc<dyn ExternalAgentRuntime> {
        let _ = ctx;
        self.create(kind)
    }
}

/// Default factory: every known kind maps to the unavailable stub.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableStubFactory;

impl ExternalRuntimeFactory for UnavailableStubFactory {
    fn create(&self, kind: ExternalAgentKind) -> Arc<dyn ExternalAgentRuntime> {
        Arc::new(UnavailableExternalRuntime::new(kind))
    }
}

/// Registry of factories keyed by [`ExternalAgentKind`].
pub struct ExternalRuntimeRegistry {
    factories: HashMap<ExternalAgentKind, Arc<dyn ExternalRuntimeFactory>>,
    fallback: Arc<dyn ExternalRuntimeFactory>,
}

impl ExternalRuntimeRegistry {
    pub fn new(fallback: Arc<dyn ExternalRuntimeFactory>) -> Self {
        Self {
            factories: HashMap::new(),
            fallback,
        }
    }

    pub fn register(&mut self, kind: ExternalAgentKind, factory: Arc<dyn ExternalRuntimeFactory>) {
        self.factories.insert(kind, factory);
    }

    fn factory_for(&self, kind: ExternalAgentKind) -> Arc<dyn ExternalRuntimeFactory> {
        self.factories
            .get(&kind)
            .cloned()
            .unwrap_or_else(|| self.fallback.clone())
    }

    /// Create without session binding (probes / bootstrap).
    pub fn create(&self, kind: ExternalAgentKind) -> Option<Arc<dyn ExternalAgentRuntime>> {
        Some(self.factory_for(kind).create(kind))
    }

    /// Create a session-bound runtime (production path).
    pub fn create_for_session(
        &self,
        kind: ExternalAgentKind,
        ctx: &ExternalRuntimeSessionContext,
    ) -> Option<Arc<dyn ExternalAgentRuntime>> {
        Some(self.factory_for(kind).create_for_session(kind, ctx))
    }
}

impl Default for ExternalRuntimeRegistry {
    fn default() -> Self {
        let mut reg = Self::new(Arc::new(UnavailableStubFactory));
        #[cfg(feature = "claude-cli-runtime")]
        {
            reg.register(
                ExternalAgentKind::ClaudeCli,
                Arc::new(super::claude_cli::ClaudeCliRuntimeFactory::from_env()),
            );
        }
        #[cfg(not(feature = "claude-cli-runtime"))]
        {
            reg.register(
                ExternalAgentKind::ClaudeCli,
                Arc::new(UnavailableStubFactory),
            );
        }
        reg
    }
}

/// Process-wide default registry.
pub fn default_registry() -> ExternalRuntimeRegistry {
    ExternalRuntimeRegistry::default()
}
