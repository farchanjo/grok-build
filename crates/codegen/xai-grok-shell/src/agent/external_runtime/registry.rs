//! Factory / registry for external agent runtimes.

use super::{ExternalAgentRuntime, UnavailableExternalRuntime};
use crate::agent::execution_backend::ExternalAgentKind;
use std::collections::HashMap;
use std::sync::Arc;

/// Creates an [`ExternalAgentRuntime`] for a given kind.
pub trait ExternalRuntimeFactory: Send + Sync {
    fn create(&self, kind: ExternalAgentKind) -> Arc<dyn ExternalAgentRuntime>;
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

    pub fn create(&self, kind: ExternalAgentKind) -> Option<Arc<dyn ExternalAgentRuntime>> {
        let factory = self.factories.get(&kind).cloned().unwrap_or_else(|| {
            // Always provide a stub for known kinds so selection fails closed
            // rather than panicking when the registry is empty.
            self.fallback.clone()
        });
        Some(factory.create(kind))
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
