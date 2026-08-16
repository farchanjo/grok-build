//! Shell-private assigned-spawn transport.
//!
//! This intentionally wraps, rather than changes, the public task event and
//! request shapes. Only `MvpAgent` can mint a sender.

use std::path::PathBuf;

use tokio::sync::mpsc;

use xai_grok_tools::implementations::grok_build::task::types::SubagentRequest;

use super::{assignment::AssignmentKey, exact_route::ExactRoute};

pub(crate) struct InternalAssignedSpawn {
    pub(crate) request: Box<SubagentRequest>,
    pub(crate) key: AssignmentKey,
    pub(crate) route: ExactRoute,
}

#[derive(Clone)]
pub(crate) struct AssignedSpawnSender {
    tx: mpsc::UnboundedSender<InternalAssignedSpawn>,
}

impl AssignedSpawnSender {
    pub(crate) fn send(&self, spawn: InternalAssignedSpawn) -> Result<(), ()> {
        self.tx.send(spawn).map_err(|_| ())
    }
}

/// Trusted goal-role boundary. It binds role identity and exact model route in
/// one operation; individual role modules never obtain an `AssignedSpawnSender`.
#[derive(Clone)]
pub(crate) struct GoalAssignedSpawnSender {
    sender: AssignedSpawnSender,
    models_manager: crate::agent::models::ModelsManager,
    inference_config: xai_grok_inference::InferenceConfig,
    grok_home: Option<PathBuf>,
    goal_id: String,
}

impl GoalAssignedSpawnSender {
    pub(crate) fn new(
        sender: AssignedSpawnSender,
        models_manager: crate::agent::models::ModelsManager,
        inference_config: xai_grok_inference::InferenceConfig,
        grok_home: Option<PathBuf>,
        goal_id: String,
    ) -> Self {
        Self {
            sender,
            models_manager,
            inference_config,
            grok_home,
            goal_id,
        }
    }

    /// Resolve canonical selection, upstream wire model, and the precise live
    /// provider route before placing the request on the private channel.
    pub(crate) fn send(
        &self,
        role: &str,
        skeptic_idx: Option<u32>,
        mut request: SubagentRequest,
    ) -> Result<(), String> {
        let requested = request
            .runtime_overrides
            .model
            .clone()
            .unwrap_or_else(|| self.models_manager.current_model_id().0.to_string());
        let models = self.models_manager.models();
        let identity = crate::agent::model_identity::resolve_model_identity(&models, &requested)
            .resolved()
            .ok_or_else(|| format!("goal role model is not uniquely resolvable: {requested}"))?;
        let canonical = identity.canonical_id;
        let context = crate::session::route_context::resolve_for_models_manager_with_selection(
            &self.inference_config,
            &self.models_manager,
            canonical.as_str(),
            self.grok_home.as_deref(),
        );
        let route =
            ExactRoute::new(canonical.clone(), identity.upstream_id, context).ok_or_else(|| {
                "goal role route did not match the resolved upstream model".to_string()
            })?;
        let key = AssignmentKey::goal(&self.goal_id, role, skeptic_idx)
            .ok_or_else(|| "goal role assignment key is invalid".to_string())?;
        request.runtime_overrides.model = Some(canonical.as_str().to_owned());
        self.sender
            .send(InternalAssignedSpawn {
                request: Box::new(request),
                key,
                route,
            })
            .map_err(|_| "subagent coordinator channel closed".to_string())
    }
}

pub(crate) fn channel() -> (
    AssignedSpawnSender,
    mpsc::UnboundedReceiver<InternalAssignedSpawn>,
) {
    let (tx, rx) = mpsc::unbounded_channel();
    (AssignedSpawnSender { tx }, rx)
}
