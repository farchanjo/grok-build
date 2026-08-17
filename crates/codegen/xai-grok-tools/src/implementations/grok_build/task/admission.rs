//! Session-scoped subagent spawn admission.
//!
//! The policy is intentionally transport-neutral: any [`SubagentBackend`]
//! can be wrapped with the same bounded, cancellation-aware decorator.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

use super::backend::SubagentBackend;
use super::types::{
    SubagentCancelOutcome, SubagentDescribeOutcome, SubagentRequest, SubagentResult,
    SubagentSnapshot, SubagentValidateTypeOutcome,
};
use xai_tool_runtime::ToolError;

pub const DEFAULT_MAX_CONCURRENT: usize = 32;

/// What happens when a non-workflow spawn reaches its session limit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LimitBehavior {
    #[default]
    Queue,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubagentLimits {
    pub max_concurrent: usize,
    pub behavior: LimitBehavior,
}

impl Default for SubagentLimits {
    fn default() -> Self {
        Self {
            max_concurrent: DEFAULT_MAX_CONCURRENT,
            behavior: LimitBehavior::Queue,
        }
    }
}

impl SubagentLimits {
    /// Resolve environment overrides once at backend construction.
    pub fn from_env() -> Self {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    pub fn from_values(max_concurrent: Option<usize>, behavior: Option<&str>) -> Self {
        let max_concurrent = max_concurrent
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_MAX_CONCURRENT);
        let behavior = match behavior {
            Some(value) if value.eq_ignore_ascii_case("fail") => LimitBehavior::Fail,
            _ => LimitBehavior::Queue,
        };
        Self {
            max_concurrent,
            behavior,
        }
    }

    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let max_concurrent = lookup("GROK_MAX_CONCURRENT_SUBAGENTS")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0);
        let raw_behavior = lookup("GROK_SUBAGENT_LIMIT_BEHAVIOR");
        if raw_behavior.as_deref().is_some_and(|value| {
            !value.eq_ignore_ascii_case("queue") && !value.eq_ignore_ascii_case("fail")
        }) {
            tracing::warn!(
                value = raw_behavior.as_deref().unwrap_or_default(),
                "GROK_SUBAGENT_LIMIT_BEHAVIOR is neither `queue` nor `fail`; keeping `queue`"
            );
        }
        Self::from_values(max_concurrent, raw_behavior.as_deref())
    }
}

/// Per-session permit pools shared by every rebuilt agent in the process.
pub struct SubagentAdmission {
    limits: SubagentLimits,
    sessions: Mutex<HashMap<String, Arc<Semaphore>>>,
    requests: Mutex<HashMap<String, QueuedRequest>>,
}

#[derive(Clone)]
struct QueuedRequest {
    parent_session_id: String,
    parent_prompt_id: Option<String>,
    cancellation: CancellationToken,
}

impl SubagentAdmission {
    pub fn new(mut limits: SubagentLimits) -> Self {
        limits.max_concurrent = limits.max_concurrent.max(1);
        Self {
            limits,
            sessions: Mutex::new(HashMap::new()),
            requests: Mutex::new(HashMap::new()),
        }
    }

    fn track(&self, request: &SubagentRequest) -> RequestGuard<'_> {
        self.requests
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                request.id.clone(),
                QueuedRequest {
                    parent_session_id: request.parent_session_id.clone(),
                    parent_prompt_id: request.parent_prompt_id.clone(),
                    cancellation: request.cancel_token.clone(),
                },
            );
        RequestGuard {
            admission: self,
            id: request.id.clone(),
        }
    }

    pub fn cancel_id(&self, id: &str) -> bool {
        let cancellation = self
            .requests
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(id)
            .map(|request| request.cancellation.clone());
        if let Some(cancellation) = cancellation {
            cancellation.cancel();
            true
        } else {
            false
        }
    }

    pub fn cancel_prompt(&self, parent_prompt_id: &str) -> usize {
        self.cancel_matching(|request| {
            request.parent_prompt_id.as_deref() == Some(parent_prompt_id)
        })
    }

    pub fn cancel_session(&self, parent_session_id: &str) -> usize {
        self.cancel_matching(|request| request.parent_session_id == parent_session_id)
    }

    fn cancel_matching(&self, predicate: impl Fn(&QueuedRequest) -> bool) -> usize {
        let cancellations: Vec<_> = self
            .requests
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .filter(|request| predicate(request))
            .map(|request| request.cancellation.clone())
            .collect();
        for cancellation in &cancellations {
            cancellation.cancel();
        }
        cancellations.len()
    }

    fn semaphore_for(&self, session_id: &str) -> Arc<Semaphore> {
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        sessions
            .entry(session_id.to_owned())
            .or_insert_with(|| Arc::new(Semaphore::new(self.limits.max_concurrent)))
            .clone()
    }

    async fn acquire(
        &self,
        session_id: &str,
        cancellation: &CancellationToken,
    ) -> Result<OwnedSemaphorePermit, AdmissionError> {
        let semaphore = self.semaphore_for(session_id);
        match self.limits.behavior {
            LimitBehavior::Fail => {
                semaphore
                    .try_acquire_owned()
                    .map_err(|_| AdmissionError::ConcurrentLimitReached {
                        limit: self.limits.max_concurrent,
                    })
            }
            LimitBehavior::Queue => {
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => Err(AdmissionError::Cancelled),
                    permit = semaphore.acquire_owned() => permit.map_err(|_| AdmissionError::Closed),
                }
            }
        }
    }

    #[cfg(test)]
    fn session_count(&self) -> usize {
        self.sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }
}

struct RequestGuard<'a> {
    admission: &'a SubagentAdmission,
    id: String,
}

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        self.admission
            .requests
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&self.id);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum AdmissionError {
    ConcurrentLimitReached { limit: usize },
    Cancelled,
    Closed,
}

impl AdmissionError {
    fn into_result(self, request: &SubagentRequest) -> SubagentResult {
        let (cancelled, error) = match self {
            Self::ConcurrentLimitReached { limit } => (
                false,
                format!(
                    "Concurrent subagent limit reached: {limit} subagents are already running for this session. Do not retry; spawning succeeds again when a running subagent finishes."
                ),
            ),
            Self::Cancelled => (
                true,
                "Subagent was cancelled while queued for admission".to_owned(),
            ),
            Self::Closed => (true, "Subagent admission closed before launch".to_owned()),
        };
        SubagentResult {
            success: false,
            cancelled,
            error: Some(error),
            subagent_id: request.id.clone(),
            child_session_id: request.id.clone(),
            ..Default::default()
        }
    }
}

/// Decorates a native or remote backend with session-scoped admission.
/// Workflow-origin children bypass this pool because workflow runs already own
/// an independent bounded agent budget; sharing the session pool can deadlock a
/// workflow behind its own children.
pub struct LimitedBackend<B> {
    inner: B,
    admission: Arc<SubagentAdmission>,
}

impl<B> LimitedBackend<B> {
    pub fn new(inner: B, admission: Arc<SubagentAdmission>) -> Self {
        Self { inner, admission }
    }
}

#[async_trait::async_trait]
impl<B: SubagentBackend> SubagentBackend for LimitedBackend<B> {
    async fn spawn(&self, request: SubagentRequest) -> Result<SubagentResult, ToolError> {
        if request.owner.is_workflow() {
            return self.inner.spawn(request).await;
        }
        let _request_guard = self.admission.track(&request);
        let permit = match self
            .admission
            .acquire(&request.parent_session_id, &request.cancel_token)
            .await
        {
            Ok(permit) => permit,
            Err(error) => return Ok(error.into_result(&request)),
        };
        if request.cancel_token.is_cancelled() {
            return Ok(AdmissionError::Cancelled.into_result(&request));
        }
        let result = self.inner.spawn(request).await;
        drop(permit);
        result
    }

    async fn query(
        &self,
        id: &str,
        block: bool,
        timeout_ms: Option<u64>,
    ) -> Option<SubagentSnapshot> {
        self.inner.query(id, block, timeout_ms).await
    }

    async fn cancel(&self, id: &str) -> SubagentCancelOutcome {
        self.inner.cancel(id).await
    }

    async fn validate_type(
        &self,
        subagent_type: &str,
        parent_session_id: &str,
    ) -> SubagentValidateTypeOutcome {
        self.inner
            .validate_type(subagent_type, parent_session_id)
            .await
    }

    async fn describe_subagent_type(
        &self,
        subagent_type: &str,
        harness_agent_type: Option<&str>,
        parent_session_id: &str,
    ) -> SubagentDescribeOutcome {
        self.inner
            .describe_subagent_type(subagent_type, harness_agent_type, parent_session_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::grok_build::task::backend::ChannelBackend;
    use crate::implementations::grok_build::task::types::{SubagentEvent, SubagentOwner};
    use tokio::sync::{mpsc, oneshot};

    fn request(id: &str, parent: &str) -> SubagentRequest {
        let (result_tx, _) = oneshot::channel();
        SubagentRequest {
            id: id.to_owned(),
            prompt: "work".to_owned(),
            description: "work".to_owned(),
            subagent_type: "general-purpose".to_owned(),
            parent_session_id: parent.to_owned(),
            parent_prompt_id: None,
            resume_from: None,
            cwd: None,
            runtime_overrides: Default::default(),
            run_in_background: false,
            surface_completion: true,
            await_to_completion: false,
            fork_context: false,
            owner: SubagentOwner::Task,
            cancel_token: CancellationToken::new(),
            result_tx,
        }
    }

    #[test]
    fn limits_parse_and_never_disable() {
        assert_eq!(
            SubagentLimits::from_values(Some(4), Some("FAIL")),
            SubagentLimits {
                max_concurrent: 4,
                behavior: LimitBehavior::Fail
            }
        );
        assert_eq!(
            SubagentLimits::from_values(Some(0), Some("queue")),
            SubagentLimits::default()
        );
        assert_eq!(
            SubagentAdmission::new(SubagentLimits {
                max_concurrent: 0,
                behavior: LimitBehavior::Queue
            })
            .limits
            .max_concurrent,
            1
        );
    }

    #[tokio::test]
    async fn queue_waits_then_releases_fifo_slot() {
        let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
        let admission = Arc::new(SubagentAdmission::new(SubagentLimits {
            max_concurrent: 1,
            behavior: LimitBehavior::Queue,
        }));
        let backend = Arc::new(LimitedBackend::new(ChannelBackend::new(tx), admission));
        let first = tokio::spawn({
            let backend = backend.clone();
            async move { backend.spawn(request("one", "session")).await }
        });
        let SubagentEvent::Spawn(first_request) = rx.recv().await.unwrap() else {
            panic!("spawn")
        };
        let second = tokio::spawn({
            let backend = backend.clone();
            async move { backend.spawn(request("two", "session")).await }
        });
        tokio::task::yield_now().await;
        assert!(rx.try_recv().is_err(), "second spawn must remain queued");
        first_request
            .result_tx
            .send(SubagentResult {
                success: true,
                ..Default::default()
            })
            .unwrap();
        first.await.unwrap().unwrap();
        let SubagentEvent::Spawn(second_request) = rx.recv().await.unwrap() else {
            panic!("spawn")
        };
        second_request
            .result_tx
            .send(SubagentResult {
                success: true,
                ..Default::default()
            })
            .unwrap();
        assert!(second.await.unwrap().unwrap().success);
    }

    #[tokio::test]
    async fn cancellation_removes_a_queued_spawn() {
        let admission = Arc::new(SubagentAdmission::new(SubagentLimits {
            max_concurrent: 1,
            behavior: LimitBehavior::Queue,
        }));
        let held = admission
            .acquire("session", &CancellationToken::new())
            .await
            .unwrap();
        let request = request("queued", "session");
        let cancel = request.cancel_token.clone();
        let _guard = admission.track(&request);
        let waiter = tokio::spawn({
            let admission = admission.clone();
            let cancel = cancel.clone();
            async move { admission.acquire("session", &cancel).await }
        });
        assert!(admission.cancel_id("queued"));
        assert!(matches!(
            waiter.await.unwrap(),
            Err(AdmissionError::Cancelled)
        ));
        drop(held);
    }

    #[tokio::test]
    async fn prompt_and_session_cancellation_reach_queued_requests() {
        let admission = SubagentAdmission::new(SubagentLimits::default());
        let mut first = request("one", "session-a");
        first.parent_prompt_id = Some("prompt".to_owned());
        let first_cancel = first.cancel_token.clone();
        let second = request("two", "session-b");
        let second_cancel = second.cancel_token.clone();
        let _first = admission.track(&first);
        let _second = admission.track(&second);
        assert_eq!(admission.cancel_prompt("prompt"), 1);
        assert!(first_cancel.is_cancelled());
        assert!(!second_cancel.is_cancelled());
        assert_eq!(admission.cancel_session("session-b"), 1);
        assert!(second_cancel.is_cancelled());
    }

    #[tokio::test]
    async fn sessions_are_isolated_and_fail_mode_recovers() {
        let admission = SubagentAdmission::new(SubagentLimits {
            max_concurrent: 1,
            behavior: LimitBehavior::Fail,
        });
        let held = admission
            .acquire("a", &CancellationToken::new())
            .await
            .unwrap();
        assert!(matches!(
            admission.acquire("a", &CancellationToken::new()).await,
            Err(AdmissionError::ConcurrentLimitReached { limit: 1 })
        ));
        let other = admission
            .acquire("b", &CancellationToken::new())
            .await
            .unwrap();
        drop(held);
        assert!(
            admission
                .acquire("a", &CancellationToken::new())
                .await
                .is_ok()
        );
        drop(other);
        assert_eq!(admission.session_count(), 2);
    }

    #[tokio::test]
    async fn workflow_children_bypass_session_admission() {
        let (tx, mut rx) = mpsc::unbounded_channel::<SubagentEvent>();
        let admission = Arc::new(SubagentAdmission::new(SubagentLimits {
            max_concurrent: 1,
            behavior: LimitBehavior::Fail,
        }));
        let backend = LimitedBackend::new(ChannelBackend::new(tx), admission.clone());
        let _held = admission
            .acquire("session", &CancellationToken::new())
            .await
            .unwrap();
        let mut workflow = request("workflow", "session");
        workflow.owner = SubagentOwner::workflow("run");
        let task = tokio::spawn(async move { backend.spawn(workflow).await });
        let SubagentEvent::Spawn(spawned) = rx.recv().await.unwrap() else {
            panic!("spawn")
        };
        spawned
            .result_tx
            .send(SubagentResult {
                success: true,
                ..Default::default()
            })
            .unwrap();
        assert!(task.await.unwrap().unwrap().success);
    }
}
