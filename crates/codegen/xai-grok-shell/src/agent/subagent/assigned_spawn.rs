//! Shell-private assigned-spawn transport.
//!
//! This intentionally wraps, rather than changes, the public task event and
//! request shapes. `MvpAgent` owns the non-cloneable mint and derives shared
//! senders only for trusted workflow/goal boundaries.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use xai_grok_tools::implementations::grok_build::task::types::SubagentRequest;

use super::{
    assignment::{AssignmentError, AssignmentKey, Assignments},
    exact_route::ExactRoute,
};

struct InternalAssignedSpawn {
    request: Box<SubagentRequest>,
    key: AssignmentKey,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum AssignedSpawnError {
    #[error(transparent)]
    Assignment(#[from] AssignmentError),
    #[error("assignment key is invalid")]
    InvalidKey,
    #[error("subagent coordinator channel closed")]
    ChannelClosed,
}

/// Private channel owner. The queue carries only an opaque key; exact routes
/// stay in this bounded store until the paired private receiver consumes them.
/// It is intentionally non-cloneable; trusted workflow/goal boundaries receive
/// a derived sender that mints stable keys internally and never exposes them.
pub(crate) struct AssignedSpawnSender {
    state: Arc<AssignedSpawnState>,
}

struct AssignedSpawnState {
    tx: mpsc::UnboundedSender<InternalAssignedSpawn>,
    assignments: Mutex<Assignments>,
}

#[derive(Clone)]
pub(crate) struct TrustedAssignedSpawnSender {
    state: Arc<AssignedSpawnState>,
    workflow_run_id: Option<String>,
}

impl AssignedSpawnState {
    fn send(
        &self,
        request: Box<SubagentRequest>,
        key: AssignmentKey,
        route: ExactRoute,
    ) -> Result<(), AssignedSpawnError> {
        // Serialize insert+enqueue with receiver take so a queued envelope can
        // never be observed before its private route capability exists.
        let mut assignments = self
            .assignments
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Err(error) = assignments.insert(key.clone(), route) {
            drop(assignments);
            crate::agent::subagent::send_failure(
                *request,
                &format!("Assigned spawn rejected: {error}"),
            );
            return Err(AssignedSpawnError::Assignment(error));
        }
        if let Err(error) = self.tx.send(InternalAssignedSpawn {
            request,
            key: key.clone(),
        }) {
            let _removed = assignments.take(&key);
            debug_assert!(_removed.is_some());
            drop(assignments);
            crate::agent::subagent::send_failure(
                *error.0.request,
                "Subagent coordinator channel closed before assigned spawn acceptance.",
            );
            return Err(AssignedSpawnError::ChannelClosed);
        }
        drop(assignments);
        Ok(())
    }
}

impl AssignedSpawnSender {
    // The raw owner intentionally exposes no route/key send operation. Shell
    // composition derives this wrapper only for trusted session boundaries.
    pub(crate) fn trusted_sender(&self) -> TrustedAssignedSpawnSender {
        TrustedAssignedSpawnSender {
            state: self.state.clone(),
            workflow_run_id: None,
        }
    }
}

impl TrustedAssignedSpawnSender {
    /// Resolve the contextual workflow key inside the trusted transport so
    /// callers never receive a key they could consume or replay.
    pub(crate) fn for_workflow(&self, run_id: &str) -> Result<Self, AssignedSpawnError> {
        if AssignmentKey::workflow(run_id, 0).is_none() {
            return Err(AssignedSpawnError::InvalidKey);
        }
        Ok(Self {
            state: self.state.clone(),
            workflow_run_id: Some(run_id.to_owned()),
        })
    }

    pub(crate) fn send_workflow(
        &self,
        sequence: u64,
        request: Box<SubagentRequest>,
        route: ExactRoute,
    ) -> Result<(), AssignedSpawnError> {
        let Some(run_id) = self.workflow_run_id.as_deref() else {
            crate::agent::subagent::send_failure(
                *request,
                "Assigned spawn rejected: workflow assignment capability is unbound",
            );
            return Err(AssignedSpawnError::InvalidKey);
        };
        let Some(key) = AssignmentKey::workflow(run_id, sequence) else {
            crate::agent::subagent::send_failure(
                *request,
                "Assigned spawn rejected: workflow assignment key is invalid",
            );
            return Err(AssignedSpawnError::InvalidKey);
        };
        self.state.send(request, key, route)
    }
}

impl std::fmt::Debug for AssignedSpawnSender {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AssignedSpawnSender")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for TrustedAssignedSpawnSender {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TrustedAssignedSpawnSender")
            .finish_non_exhaustive()
    }
}

/// Paired private consumer; not exposed outside the coordinator owner.
pub(crate) struct AssignedSpawnReceiver {
    rx: mpsc::UnboundedReceiver<InternalAssignedSpawn>,
    state: Arc<AssignedSpawnState>,
}

impl AssignedSpawnReceiver {
    /// Take one private assignment. The opaque key is sealed into
    /// [`AssignedRoute`] here so other shell modules never observe or steal it.
    pub(crate) async fn recv(&mut self) -> Option<(Box<SubagentRequest>, super::AssignedRoute)> {
        while let Some(spawn) = self.rx.recv().await {
            let route = self
                .state
                .assignments
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take(&spawn.key);
            if let Some(route) = route {
                return Some((spawn.request, super::AssignedRoute::new(spawn.key, route)));
            }
            tracing::error!(
                assignment_key = spawn.key.as_str(),
                "assigned spawn arrived without its trusted exact-route capability"
            );
            crate::agent::subagent::send_failure(
                *spawn.request,
                "Assigned exact-route capability was missing or already consumed.",
            );
        }
        None
    }
}

impl Drop for AssignedSpawnReceiver {
    fn drop(&mut self) {
        // Close before draining so a concurrent mint either lands in this
        // drain or deterministically fails and rolls its store entry back.
        self.rx.close();
        while let Ok(spawn) = self.rx.try_recv() {
            let _removed = self
                .state
                .assignments
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take(&spawn.key);
            debug_assert!(_removed.is_some());
            crate::agent::subagent::send_failure(
                *spawn.request,
                "Subagent coordinator stopped before assigned spawn acceptance.",
            );
        }
    }
}

/// Trusted goal-role boundary. It binds role identity and exact model route in
/// one operation; individual role modules never obtain an `AssignedSpawnSender`.
#[derive(Clone)]
pub(crate) struct GoalAssignedSpawnSender {
    state: Arc<AssignedSpawnState>,
    models_manager: crate::agent::models::ModelsManager,
    inference_config: xai_grok_inference::InferenceConfig,
    grok_home: Option<PathBuf>,
    goal_id: String,
}

impl std::fmt::Debug for GoalAssignedSpawnSender {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GoalAssignedSpawnSender")
            .field("goal_id", &self.goal_id)
            .finish_non_exhaustive()
    }
}

impl GoalAssignedSpawnSender {
    pub(crate) fn new(
        sender: &TrustedAssignedSpawnSender,
        models_manager: crate::agent::models::ModelsManager,
        inference_config: xai_grok_inference::InferenceConfig,
        grok_home: Option<PathBuf>,
        goal_id: String,
    ) -> Self {
        Self {
            state: sender.state.clone(),
            models_manager,
            inference_config,
            grok_home,
            goal_id,
        }
    }

    /// Resolve canonical selection, upstream wire model, the precise live
    /// provider route, and the stable role key before placing the request on
    /// the private channel. The role caller never sees the key.
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
        )
        .map_err(|e| format!("provider route unusable for assigned spawn: {e}"))?;
        let route =
            ExactRoute::new(canonical.clone(), identity.upstream_id, context).ok_or_else(|| {
                "goal role route did not match the resolved upstream model".to_string()
            })?;
        let key = AssignmentKey::goal(&self.goal_id, role, skeptic_idx)
            .ok_or_else(|| "goal role assignment key is invalid".to_string())?;
        request.runtime_overrides.model = Some(canonical.as_str().to_owned());
        self.state
            .send(Box::new(request), key, route)
            .map_err(|error| error.to_string())
    }
}

pub(crate) fn channel() -> (AssignedSpawnSender, AssignedSpawnReceiver) {
    let (tx, rx) = mpsc::unbounded_channel();
    let state = Arc::new(AssignedSpawnState {
        tx,
        assignments: Mutex::new(Assignments::default()),
    });
    (
        AssignedSpawnSender {
            state: state.clone(),
        },
        AssignedSpawnReceiver { rx, state },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;
    use tokio_util::sync::CancellationToken;
    use xai_grok_inference::{
        ProviderRouteContext, RouteApiSurface, RouteAuthority, RouteCredentialRoute,
        RouteProviderKind,
    };
    use xai_grok_models::{CanonicalModelId, UpstreamModelId};
    use xai_grok_tools::implementations::grok_build::task::types::{
        SubagentOwner, SubagentRuntimeOverrides,
    };

    fn route(instance_len: usize) -> ExactRoute {
        let upstream = UpstreamModelId::new("gpt-4o").unwrap();
        ExactRoute::new(
            CanonicalModelId::new("openai:gpt-4o").unwrap(),
            upstream,
            ProviderRouteContext::builder()
                .instance_id("x".repeat(instance_len))
                .incarnation("01234567-89ab-cdef-0123-456789abcdef")
                .provider_kind(RouteProviderKind::OpenAi)
                .api_surface(RouteApiSurface::OpenAiPlatform)
                .credential_route(RouteCredentialRoute::ApiKey)
                .registry_generation(1)
                .binding_generation(1)
                .authority(RouteAuthority::Authoritative)
                .model_partition("gpt-4o")
                .build()
                .unwrap(),
        )
        .unwrap()
    }

    fn request(id: &str) -> Box<SubagentRequest> {
        let (result_tx, result_rx) = oneshot::channel();
        std::mem::forget(result_rx);
        request_with_sender(id, result_tx)
    }

    fn request_with_result(
        id: &str,
    ) -> (
        Box<SubagentRequest>,
        oneshot::Receiver<xai_grok_tools::implementations::grok_build::task::types::SubagentResult>,
    ) {
        let (result_tx, result_rx) = oneshot::channel();
        (request_with_sender(id, result_tx), result_rx)
    }

    fn request_with_sender(
        id: &str,
        result_tx: oneshot::Sender<
            xai_grok_tools::implementations::grok_build::task::types::SubagentResult,
        >,
    ) -> Box<SubagentRequest> {
        Box::new(SubagentRequest {
            id: id.to_string(),
            prompt: "test".into(),
            description: "test".into(),
            subagent_type: "explore".into(),
            parent_session_id: "parent".into(),
            parent_prompt_id: None,
            resume_from: None,
            cwd: None,
            runtime_overrides: SubagentRuntimeOverrides::default(),
            run_in_background: false,
            surface_completion: false,
            await_to_completion: true,
            fork_context: false,
            owner: SubagentOwner::Task,
            cancel_token: CancellationToken::new(),
            result_tx,
        })
    }

    #[tokio::test]
    async fn invalid_workflow_key_is_rejected_without_inserting() {
        let (sender, receiver) = channel();
        let trusted = sender.trusted_sender();
        drop(sender);
        assert!(matches!(
            trusted.for_workflow(&"x".repeat(512)),
            Err(AssignedSpawnError::InvalidKey)
        ));
        let (unbound_request, unbound_result) = request_with_result("unbound");
        assert_eq!(
            trusted.send_workflow(0, unbound_request, route(8)),
            Err(AssignedSpawnError::InvalidKey)
        );
        assert!(
            unbound_result
                .await
                .unwrap()
                .error
                .as_deref()
                .is_some_and(|error| error.contains("workflow assignment capability is unbound"))
        );
        assert_eq!(
            receiver
                .state
                .assignments
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn receiver_drop_rejects_and_releases_queued_assignments() {
        let (sender, receiver) = channel();
        let (request, result) = request_with_result("queued");
        let trusted = sender.trusted_sender();
        trusted
            .for_workflow("run")
            .unwrap()
            .send_workflow(0, request, route(8))
            .unwrap();
        let state = trusted.state.clone();
        drop(sender);
        drop(trusted);
        drop(receiver);
        assert!(
            result
                .await
                .unwrap()
                .error
                .as_deref()
                .is_some_and(|error| error.contains("coordinator stopped"))
        );
        assert_eq!(
            state
                .assignments
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn live_sender_enforces_capacity_duplicate_and_byte_cap_without_leaks() {
        let (sender, receiver) = channel();
        let trusted = sender.trusted_sender().for_workflow("run").unwrap();
        drop(sender);
        for index in 0..super::super::assignment::MAX_ASSIGNMENT_ENTRIES {
            trusted
                .send_workflow(index as u64, request(&format!("child-{index}")), route(8))
                .unwrap();
        }
        let (full_request, full_result) = request_with_result("full");
        assert!(matches!(
            trusted.send_workflow(2_000, full_request, route(8)),
            Err(AssignedSpawnError::Assignment(AssignmentError::Full))
        ));
        assert!(
            full_result
                .await
                .unwrap()
                .error
                .as_deref()
                .is_some_and(|error| error.contains("capacity exhausted"))
        );
        assert_eq!(
            receiver
                .state
                .assignments
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len(),
            super::super::assignment::MAX_ASSIGNMENT_ENTRIES
        );
        drop(trusted);
        drop(receiver);
        let (sender, receiver) = channel();
        let trusted = sender.trusted_sender().for_workflow("duplicate").unwrap();
        drop(sender);
        trusted
            .send_workflow(0, request("first"), route(8))
            .unwrap();
        let (duplicate_request, duplicate_result) = request_with_result("second");
        assert!(matches!(
            trusted.send_workflow(0, duplicate_request, route(8)),
            Err(AssignedSpawnError::Assignment(AssignmentError::Duplicate))
        ));
        assert!(
            duplicate_result
                .await
                .unwrap()
                .error
                .as_deref()
                .is_some_and(|error| error.contains("already exists"))
        );
        assert_eq!(
            receiver
                .state
                .assignments
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len(),
            1
        );
        drop(trusted);
        drop(receiver);
        let (sender, receiver) = channel();
        let trusted = sender.trusted_sender().for_workflow("large").unwrap();
        drop(sender);
        let previous_bytes = receiver
            .state
            .assignments
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .set_bytes_for_test(super::super::assignment::MAX_ASSIGNMENT_BYTES);
        let (too_large_request, too_large_result) = request_with_result("too-large");
        assert!(matches!(
            trusted.send_workflow(0, too_large_request, route(8)),
            Err(AssignedSpawnError::Assignment(AssignmentError::TooLarge))
        ));
        assert!(
            too_large_result
                .await
                .unwrap()
                .error
                .as_deref()
                .is_some_and(|error| error.contains("byte cap exceeded"))
        );
        {
            let assignments = receiver
                .state
                .assignments
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            assert_eq!(assignments.len(), 0);
            assert_eq!(
                assignments.bytes(),
                super::super::assignment::MAX_ASSIGNMENT_BYTES
            );
            assert_eq!(previous_bytes, 0);
        }
        let _ = receiver
            .state
            .assignments
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .set_bytes_for_test(previous_bytes);
        drop(trusted);
        drop(receiver);
    }

    #[tokio::test]
    async fn failed_send_reports_failure_releases_assignment_and_consumption_is_single_use() {
        let (sender, receiver) = channel();
        let trusted = sender.trusted_sender().for_workflow("run").unwrap();
        let state = trusted.state.clone();
        drop(sender);
        drop(receiver);
        let (closed_request, closed_result) = request_with_result("closed");
        assert_eq!(
            trusted.send_workflow(1, closed_request, route(8)),
            Err(AssignedSpawnError::ChannelClosed)
        );
        assert!(
            closed_result
                .await
                .unwrap()
                .error
                .as_deref()
                .is_some_and(|error| error.contains("coordinator channel closed"))
        );
        assert_eq!(
            state
                .assignments
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len(),
            0
        );

        let (sender, mut receiver) = channel();
        let trusted = sender.trusted_sender().for_workflow("run").unwrap();
        drop(sender);
        let key = AssignmentKey::workflow("run", 2).unwrap();
        trusted.send_workflow(2, request("live"), route(8)).unwrap();
        let (_, assigned) = receiver.recv().await.unwrap();
        assert_eq!(assigned.key_for_test(), &key);
        assert!(
            trusted
                .state
                .assignments
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take_without_accounting_for_test(&key)
                .is_none(),
            "the private receiver cannot replay an already-consumed key"
        );
        assert_eq!(
            trusted
                .state
                .assignments
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len(),
            0
        );
    }
}
