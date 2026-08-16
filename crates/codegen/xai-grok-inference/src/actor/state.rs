//! Actor-internal state.
//!
//! All fields are touched only from the actor task, so no mutex /
//! atomic synchronization is needed -- the actor's command-loop
//! serialization gives us a "single-threaded with shared state"
//! discipline matching the hunk-tracker pattern.

use std::collections::HashMap;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::pacing::InferencePacer;
use crate::config::{InferenceConfig, RetryPolicy};
use crate::route_context::{ProviderRouteContext, RouteContextUpdate};
use crate::types::RequestId;

/// In-flight request bookkeeping.
///
/// `cancel_token` is owned by the actor (cloned into the spawned
/// per-request task). The completion oneshot is moved into the
/// per-request task at spawn time and is therefore not stored here.
pub(crate) struct ActiveRequest {
    pub(crate) cancel_token: CancellationToken,
}

/// Actor-owned state.
pub(crate) struct ActorState {
    pub(crate) active_requests: HashMap<RequestId, ActiveRequest>,
    pub(crate) config: InferenceConfig,
    /// Explicit route context when known; otherwise derived from config.
    pub(crate) route_context: Option<ProviderRouteContext>,
    pub(crate) retry_policy: RetryPolicy,
    pub(crate) inference_pacer: Arc<InferencePacer>,
}

impl ActorState {
    pub(crate) fn new(config: InferenceConfig, retry_policy: RetryPolicy) -> Self {
        Self {
            active_requests: HashMap::new(),
            config,
            route_context: None,
            retry_policy,
            inference_pacer: InferencePacer::shared(),
        }
    }

    /// Register a newly-spawned request. Returns the previous entry if
    /// the same `request_id` was already in flight (callers should
    /// cancel the previous token before overwriting).
    pub(crate) fn register(
        &mut self,
        request_id: RequestId,
        active: ActiveRequest,
    ) -> Option<ActiveRequest> {
        self.active_requests.insert(request_id, active)
    }

    /// Remove a request from the active set without cancelling its
    /// token. Used by the cleanup signal sent from per-request tasks
    /// when they exit normally.
    pub(crate) fn remove(&mut self, request_id: &RequestId) -> Option<ActiveRequest> {
        self.active_requests.remove(request_id)
    }

    /// Cancel and remove an in-flight request.
    pub(crate) fn cancel(&mut self, request_id: &RequestId) -> bool {
        if let Some(active) = self.active_requests.remove(request_id) {
            active.cancel_token.cancel();
            true
        } else {
            false
        }
    }

    /// Replace the default config. The next request submitted without
    /// an override will use this. Clears any explicit route context so a
    /// stale account context cannot survive a bare config refresh.
    pub(crate) fn update_config(&mut self, config: InferenceConfig) {
        self.apply_config_update(config, RouteContextUpdate::DeriveLegacy);
    }

    /// Atomically replace config and route context together.
    pub(crate) fn apply_config_update(
        &mut self,
        config: InferenceConfig,
        route: RouteContextUpdate,
    ) {
        self.config = config;
        match route {
            RouteContextUpdate::DeriveLegacy => {
                self.route_context = None;
            }
            RouteContextUpdate::Replace(ctx) => {
                self.route_context = Some(ctx);
            }
            RouteContextUpdate::Clear => {
                self.route_context = None;
            }
        }
    }

    /// Effective route context for the next request.
    pub(crate) fn effective_route_context(&self) -> ProviderRouteContext {
        self.route_context
            .clone()
            .unwrap_or_else(|| ProviderRouteContext::legacy_from_config(&self.config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ApiBackend;
    use indexmap::IndexMap;

    /// Minimal config builder for tests in this module.
    fn cfg() -> InferenceConfig {
        InferenceConfig {
            api_key: None,
            base_url: "https://example.test".into(),
            model: "test-model".into(),
            max_completion_tokens: None,
            temperature: None,
            top_p: None,
            openrouter_fallback_models: Vec::new(),
            openrouter_provider_preferences: None,
            openrouter_plugins: Vec::new(),
            openrouter_pacing: false,
            zai_tool_stream: false,
            zai_thinking: None,
            api_backend: ApiBackend::ChatCompletions,
            include_message_model_id: true,
            auth_scheme: Default::default(),
            extra_headers: IndexMap::new(),
            context_window: 8192,
            force_http1: false,
            max_retries: None,
            stream_tool_calls: false,
            idle_timeout_secs: None,
            reasoning_effort: None,
            origin_client: None,
            client_identifier: None,
            deployment_id: None,
            user_id: None,
            client_version: None,
            provider_identity: crate::config::ProviderIdentity::default(),
            attribution_callback: None,
            bearer_resolver: None,
            supports_backend_search: false,
            supports_native_schema: None,
            supports_strict_tools: None,
            compactions_remaining: None,
            compaction_at_tokens: None,
            doom_loop_recovery: None,
            header_injector: None,
            supports_image_input: None,
            supports_audio_input: None,
            supports_video_input: None,
        }
    }

    #[test]
    fn cancel_unknown_request_returns_false() {
        let mut state = ActorState::new(cfg(), RetryPolicy::default());
        assert!(!state.cancel(&RequestId::from("unknown")));
    }

    #[test]
    fn register_then_cancel_removes() {
        let mut state = ActorState::new(cfg(), RetryPolicy::default());
        let id = RequestId::from("req-1");
        state.register(
            id.clone(),
            ActiveRequest {
                cancel_token: CancellationToken::new(),
            },
        );
        assert_eq!(state.active_requests.len(), 1);
        assert!(state.cancel(&id));
        assert_eq!(state.active_requests.len(), 0);
    }

    #[test]
    fn register_returns_previous_when_same_id() {
        let mut state = ActorState::new(cfg(), RetryPolicy::default());
        let id = RequestId::from("req-1");
        let first = ActiveRequest {
            cancel_token: CancellationToken::new(),
        };
        let second = ActiveRequest {
            cancel_token: CancellationToken::new(),
        };
        assert!(state.register(id.clone(), first).is_none());
        assert!(state.register(id.clone(), second).is_some());
    }
}
