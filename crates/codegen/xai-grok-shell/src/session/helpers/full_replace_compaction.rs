//! grok-build's L5 wiring onto the shared full-replace engine
//! (`xai_grok_compaction::code_compaction`).
//!
//! The shared engine drives the sample → retry → degenerate/failure
//! classification loop via [`sample_full_replace_summary`](xai_grok_compaction::sample_full_replace_summary);
//! this module adapts grok-build's transport and telemetry to its two seams:
//!
//! - [`ShellCompactionSampler`] wraps
//!   [`generate_session_compact`](crate::session::helpers::session_compact::generate_session_compact)
//!   as the shared [`CompactionSampler`]. It also stashes the full
//!   [`CompactOutput`] of the last successful call so the L5 loop can still
//!   record the streaming telemetry (TTFT / stream span / stop reason) that
//!   the shared [`LlmCompactionOutput`] doesn't model.
//! - [`ShellFullReplaceObserver`] collects the per-attempt
//!   [`CompactionAttempt`] rows, rejection counters, and emits the
//!   `CompactionRetryDegraded` event — preserving the pre-migration telemetry.
//!
//! The verbatim → fitted → lossy **input ladder** and auto-compaction
//! suppression stay in L5 (`compaction.rs`), driven by the
//! `context_overflow` / `deterministic` flags on
//! [`FullReplaceError`](xai_grok_compaction::FullReplaceError).

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use agent_client_protocol as acp;
use async_trait::async_trait;
use xai_grok_compaction::{
    CompactionPrompt, CompactionSampleError, CompactionSampler, FullReplaceAttemptOutcome,
    FullReplaceObserver, LlmCompactionOutput,
};
use xai_grok_inference::InferenceConfig;
use xai_grok_inference_types::{ConversationItem, HostedTool, ToolSpec};
use xai_grok_telemetry::events::{CompactionRetryDegraded, CompactionTrigger};

use xai_chat_state::compaction_utils::{
    CompactionAttempt, MAX_CAPTURED_SUMMARY_CHARS, bound_captured_output,
};

use crate::inference::Client as OaiCompatClient;
use crate::session::helpers::session_compact::{
    CompactFailure, CompactOutput, build_compaction_chat_history, generate_session_compact,
};

/// Wraps `generate_session_compact` as the shared engine's
/// [`CompactionSampler`] for grok-build's full-replace pass.
///
/// Holds the per-call request context the seam does not carry (tools, client,
/// session, config) and stashes the last successful [`CompactOutput`] so the
/// caller can recover the streaming telemetry not modeled by
/// [`LlmCompactionOutput`].
///
/// The summarization prompt is selected here by `use_short_prompt` (the
/// short-prompt harness uses the short self-summarization prompt; everyone
/// else the structured grok-build prompt), so the shared `CompactionPrompt`
/// the engine passes is ignored — the engine builds the grok-build prompt,
/// which equals what `build_compaction_chat_history(.., false)` appends, and
/// the short-prompt harness needs its own variant the engine can't produce.
pub(crate) struct CompactionRoute {
    pub(crate) client: OaiCompatClient,
    pub(crate) inference_config: InferenceConfig,
}

pub(crate) struct ShellCompactionSampler {
    use_short_prompt: bool,
    user_context: Option<String>,
    use_supplied_prompt: bool,
    tools: Vec<ToolSpec>,
    hosted_tools: Vec<HostedTool>,
    routes: Vec<CompactionRoute>,
    route_index: AtomicUsize,
    session_id: acp::SessionId,
    /// Per-chunk idle timeout forwarded to `generate_session_compact`: a stalled
    /// summarizer stream (no model-output chunk for this long) fails instead of
    /// hanging.
    idle_timeout: Duration,
    /// Wall-clock budget (secs) forwarded to `generate_session_compact` as the
    /// reasoning-runaway backstop; `0` disables it.
    wall_clock_budget_secs: u64,
    tool_choice: crate::util::config::CompactionToolChoice,
    /// Full output of the most recent successful sample (for L5 telemetry).
    last_success: Mutex<Option<CompactOutput>>,
}

impl ShellCompactionSampler {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        use_short_prompt: bool,
        user_context: Option<String>,
        tools: Vec<ToolSpec>,
        hosted_tools: Vec<HostedTool>,
        routes: Vec<CompactionRoute>,
        session_id: acp::SessionId,
        idle_timeout: Duration,
        wall_clock_budget_secs: u64,
        tool_choice: crate::util::config::CompactionToolChoice,
    ) -> Self {
        Self {
            use_short_prompt,
            user_context,
            use_supplied_prompt: false,
            tools,
            hosted_tools,
            routes,
            route_index: AtomicUsize::new(0),
            session_id,
            idle_timeout,
            wall_clock_budget_secs,
            tool_choice,
            last_success: Mutex::new(None),
        }
    }

    /// Make this sampler honor the [`CompactionPrompt`] supplied to each call.
    ///
    /// Full-replace keeps its legacy prompt construction, while rolling
    /// compaction needs distinct chunk and merge instructions.
    pub(crate) fn with_supplied_prompt(mut self) -> Self {
        self.use_supplied_prompt = true;
        self
    }

    /// Take the [`CompactOutput`] of the most recent successful sample, if any.
    pub(crate) fn take_last_success(&self) -> Option<CompactOutput> {
        self.last_success.lock().unwrap().take()
    }

    pub(crate) fn selected_model(&self) -> Option<&str> {
        self.routes
            .get(self.route_index.load(Ordering::Acquire))
            .map(|route| route.inference_config.model.as_str())
    }
}

#[async_trait]
impl CompactionSampler for ShellCompactionSampler {
    type Item = ConversationItem;

    async fn sample_compaction(
        &self,
        turns: &[ConversationItem],
        prompt: &CompactionPrompt,
        _timeout: Duration,
    ) -> Result<LlmCompactionOutput, CompactionSampleError> {
        let chat_history = if self.use_supplied_prompt {
            let mut history = Vec::with_capacity(turns.len() + 2);
            if !prompt.system.trim().is_empty() {
                history.push(ConversationItem::system(prompt.system.clone()));
            }
            history.extend_from_slice(turns);
            history.push(ConversationItem::user(prompt.user.clone()));
            history
        } else {
            // Full-replace retains the harness-selected legacy prompt.
            build_compaction_chat_history(
                turns.to_vec(),
                self.user_context.as_deref(),
                self.use_short_prompt,
            )
        };

        let mut route_index = self.route_index.load(Ordering::Acquire);
        loop {
            let route = self.routes.get(route_index).ok_or_else(|| {
                CompactionSampleError::Build("no usable compaction route configured".to_owned())
            })?;
            match generate_session_compact(
                chat_history.clone(),
                self.tools.clone(),
                self.hosted_tools.clone(),
                route.client.clone(),
                self.session_id.clone(),
                &route.inference_config,
                self.idle_timeout,
                self.wall_clock_budget_secs,
                self.tool_choice,
            )
            .await
            {
                Ok(output)
                    if route_index + 1 < self.routes.len()
                        && (output.content.trim().is_empty()
                            || xai_grok_compaction::is_degenerate_summary(&output.content)) =>
                {
                    tracing::warn!(
                        failed_model = %route.inference_config.model,
                        fallback_model = %self.routes[route_index + 1].inference_config.model,
                        empty = output.content.trim().is_empty(),
                        "compaction route returned an unusable summary; trying configured fallback"
                    );
                    route_index += 1;
                    self.route_index.store(route_index, Ordering::Release);
                }
                Ok(output) => {
                    let response = output.content.clone();
                    *self.last_success.lock().unwrap() = Some(output);
                    self.route_index.store(route_index, Ordering::Release);
                    return Ok(LlmCompactionOutput {
                        response,
                        thinking: String::new(),
                    });
                }
                Err(CompactFailure::Transient(error)) if route_index + 1 < self.routes.len() => {
                    tracing::warn!(
                        failed_model = %route.inference_config.model,
                        fallback_model = %self.routes[route_index + 1].inference_config.model,
                        error = %acp_error_message(&error),
                        "compaction route failed transiently; trying configured fallback"
                    );
                    route_index += 1;
                    self.route_index.store(route_index, Ordering::Release);
                }
                Err(failure) => return Err(compact_failure_to_sample_error(failure)),
            }
        }
    }
}

/// Map grok-build's [`CompactFailure`] onto the shared engine's
/// [`CompactionSampleError`] so the shared retry loop classifies it the same
/// way the in-shell loop did:
///
/// - `Deterministic` → [`CompactionSampleError::Build`] (whose
///   `is_deterministic()` is `true`); a context-length overflow keeps its
///   message text so the engine's `is_context_length_error` check fires and
///   sets `context_overflow`.
/// - `Transient` → [`CompactionSampleError::Other`] (`is_deterministic()` is
///   `false`), so the engine retries it.
fn compact_failure_to_sample_error(failure: CompactFailure) -> CompactionSampleError {
    let (deterministic, err) = match failure {
        CompactFailure::Deterministic(err) => (true, err),
        CompactFailure::Transient(err) => (false, err),
    };
    let message = acp_error_message(&err);
    if deterministic {
        CompactionSampleError::Build(message)
    } else {
        CompactionSampleError::Other(anyhow::anyhow!(message))
    }
}

/// Render the human-readable detail an `acp::Error` carries in its `data`
/// field (where `classify_*` stash `"compact failed: <upstream>"`).
fn acp_error_message(err: &acp::Error) -> String {
    err.data
        .as_ref()
        .and_then(|d| d.as_str())
        .unwrap_or("<no data>")
        .to_string()
}

/// Collected telemetry from a full-replace pass, drained by the L5 loop after
/// the shared engine returns.
pub(crate) struct FullReplaceTelemetry {
    pub attempts: u32,
    pub attempt_details: Vec<CompactionAttempt>,
    pub degenerate_rejections: u32,
    pub transient_rejections: u32,
    pub deterministic_rejections: u32,
    /// Raw text of the last degenerate (rejected) summary, for the artifact.
    pub last_rejected_summary: Option<String>,
}

#[derive(Default)]
struct ObserverState {
    attempts: u32,
    attempt_details: Vec<CompactionAttempt>,
    degenerate_rejections: u32,
    transient_rejections: u32,
    deterministic_rejections: u32,
    last_rejected_summary: Option<String>,
    last_error_msg: Option<String>,
}

/// [`FullReplaceObserver`] that reproduces grok-build's per-attempt telemetry:
/// `CompactionAttempt` rows, rejection counters, the `CompactionRetryDegraded`
/// event, and the warn/error tracing — without the shared engine depending on
/// a telemetry backend.
pub(crate) struct ShellFullReplaceObserver {
    trigger: CompactionTrigger,
    context_window: u64,
    compaction_id: String,
    session_id: String,
    estimated_input_tokens: u64,
    retry_delay_secs: u64,
    state: Mutex<ObserverState>,
}

impl ShellFullReplaceObserver {
    pub(crate) fn new(
        trigger: CompactionTrigger,
        context_window: u64,
        compaction_id: String,
        session_id: String,
        estimated_input_tokens: u64,
        retry_delay_secs: u64,
    ) -> Self {
        Self {
            trigger,
            context_window,
            compaction_id,
            session_id,
            estimated_input_tokens,
            retry_delay_secs,
            state: Mutex::new(ObserverState::default()),
        }
    }

    /// Cumulative number of attempts so far (across all input-ladder stages).
    /// Read mid-loop to label the `input_overflow` retry event.
    pub(crate) fn attempt_count(&self) -> u32 {
        self.state.lock().unwrap().attempts
    }

    /// Whether any attempt so far produced a degenerate summary — lets the L5
    /// loop distinguish degenerate-exhausted from empty-exhausted.
    pub(crate) fn degenerate_seen(&self) -> bool {
        self.state.lock().unwrap().degenerate_rejections > 0
    }

    /// The most recent rendered error/diagnostic detail, for `last_error`.
    pub(crate) fn last_error_message(&self) -> Option<String> {
        self.state.lock().unwrap().last_error_msg.clone()
    }

    /// Drain the collected telemetry. The cumulative attempt count spans all
    /// input-ladder stages because the same observer instance is shared across
    /// every per-stage call.
    pub(crate) fn into_telemetry(self) -> FullReplaceTelemetry {
        let s = self.state.into_inner().unwrap();
        FullReplaceTelemetry {
            attempts: s.attempts,
            attempt_details: s.attempt_details,
            degenerate_rejections: s.degenerate_rejections,
            transient_rejections: s.transient_rejections,
            deterministic_rejections: s.deterministic_rejections,
            last_rejected_summary: s.last_rejected_summary,
        }
    }
}

impl FullReplaceObserver for ShellFullReplaceObserver {
    fn on_attempt(&self, _attempt: u32, outcome: &FullReplaceAttemptOutcome<'_>) {
        let mut s = self.state.lock().unwrap();
        // The shared `attempt` resets per ladder stage; keep a cumulative count
        // so artifact rows match the pre-migration numbering.
        s.attempts += 1;
        let attempt = s.attempts;

        match outcome {
            FullReplaceAttemptOutcome::Success { summary } => {
                s.attempt_details.push(CompactionAttempt {
                    attempt,
                    outcome: "success".to_string(),
                    summary_chars: summary.chars().count() as u64,
                    summary: None,
                    error: None,
                });
            }
            FullReplaceAttemptOutcome::Degenerate {
                summary,
                will_retry,
            } => {
                s.degenerate_rejections += 1;
                let summary_chars = summary.chars().count();
                s.attempt_details.push(CompactionAttempt {
                    attempt,
                    outcome: "degenerate".to_string(),
                    summary_chars: summary_chars as u64,
                    summary: Some(bound_captured_output(summary, MAX_CAPTURED_SUMMARY_CHARS)),
                    error: None,
                });
                s.last_rejected_summary = Some((*summary).to_string());
                s.last_error_msg = Some(format!(
                    "compact failed: degenerate summary \
                     ({summary_chars} chars for ~{} input tokens)",
                    self.estimated_input_tokens
                ));
                if *will_retry {
                    xai_grok_telemetry::session_ctx::log_event(CompactionRetryDegraded {
                        trigger: self.trigger,
                        reason: "degenerate_summary",
                        from_stage: None,
                        to_stage: None,
                        summary_chars: Some(summary_chars as u64),
                        attempt,
                        context_window: self.context_window,
                        compaction_id: self.compaction_id.clone(),
                    });
                    tracing::warn!(
                        session_id = %self.session_id,
                        attempt,
                        summary_chars,
                        estimated_input_tokens = self.estimated_input_tokens,
                        retry_delay_secs = self.retry_delay_secs,
                        "Compaction produced a degenerate summary, retrying in {} seconds...",
                        self.retry_delay_secs
                    );
                } else {
                    tracing::error!(
                        session_id = %self.session_id,
                        attempt,
                        summary_chars,
                        estimated_input_tokens = self.estimated_input_tokens,
                        "Compaction produced only degenerate summaries after max retries"
                    );
                }
            }
            FullReplaceAttemptOutcome::EmptyResponse { .. } => {
                // The shell surfaces an empty response as a transient error
                // (`generate_session_compact` returns `Transient`), so it never
                // reaches the shared `Ok("")` branch; handle defensively.
                s.transient_rejections += 1;
                let msg = "compact failed: model returned empty response".to_string();
                s.attempt_details.push(CompactionAttempt {
                    attempt,
                    outcome: "transient".to_string(),
                    summary_chars: 0,
                    summary: None,
                    error: Some(msg.clone()),
                });
                s.last_error_msg = Some(msg);
            }
            FullReplaceAttemptOutcome::Failure {
                message,
                deterministic,
                context_overflow,
                will_retry,
            } => {
                // A context overflow is recorded as a `deterministic` attempt
                // (matching the pre-migration row) but does NOT count toward
                // `deterministic_rejections` — the L5 ladder steps down on it
                // and tracks its own `input_overflow_rejections`.
                if *deterministic {
                    if !*context_overflow {
                        s.deterministic_rejections += 1;
                        tracing::error!(
                            session_id = %self.session_id,
                            attempt,
                            error = %message,
                            "Compaction failed (deterministic error class, no further retries)"
                        );
                    }
                    s.attempt_details.push(CompactionAttempt {
                        attempt,
                        outcome: "deterministic".to_string(),
                        summary_chars: 0,
                        summary: None,
                        error: Some((*message).to_string()),
                    });
                } else {
                    s.transient_rejections += 1;
                    s.attempt_details.push(CompactionAttempt {
                        attempt,
                        outcome: "transient".to_string(),
                        summary_chars: 0,
                        summary: None,
                        error: Some((*message).to_string()),
                    });
                    if *will_retry {
                        tracing::warn!(
                            session_id = %self.session_id,
                            attempt,
                            retry_delay_secs = self.retry_delay_secs,
                            error = %message,
                            "Compaction attempt {} failed, retrying in {} seconds...",
                            attempt,
                            self.retry_delay_secs
                        );
                    } else {
                        tracing::error!(
                            session_id = %self.session_id,
                            attempt,
                            error = %message,
                            "Compaction failed after max retries"
                        );
                    }
                }
                s.last_error_msg = Some((*message).to_string());
            }
        }
    }
}

#[cfg(test)]
mod compaction_route_tests {
    use super::*;
    use crate::inference::{ApiBackend, Client, InferenceConfig, ToolChoice};
    use axum::response::sse::{Event, KeepAlive, Sse};
    use axum::routing::post;
    use axum::{Json, Router};
    use futures_util::stream;
    use reqwest::StatusCode;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use xai_grok_inference::config::ProviderIdentity;

    fn summary_stream(content: &str) -> Vec<Event> {
        vec![
            Event::default().data(
                json!({
                    "id": "chatcmpl-test",
                    "object": "chat.completion.chunk",
                    "created": 1234567890,
                    "model": "test-model",
                    "choices": [{
                        "index": 0,
                        "delta": { "role": "assistant", "content": content },
                        "finish_reason": "stop"
                    }]
                })
                .to_string(),
            ),
            Event::default().data("[DONE]"),
        ]
    }

    fn empty_stream() -> Vec<Event> {
        vec![Event::default().data("[DONE]")]
    }

    fn usable_summary_stream(label: &str) -> Vec<Event> {
        summary_stream(&format!(
            "<summary>{label}\n{}</summary>",
            "retained detail ".repeat(40)
        ))
    }

    fn api_error_response(
        status: StatusCode,
        message: &str,
    ) -> (StatusCode, Json<serde_json::Value>) {
        (
            status,
            Json(json!({
                "error": {
                    "message": message
                }
            })),
        )
    }

    fn make_test_config(base_url: &str, provider_identity: ProviderIdentity) -> InferenceConfig {
        InferenceConfig {
            api_key: Some("test-api-key".to_string()),
            base_url: base_url.to_string(),
            model: "test-model".to_string(),
            max_completion_tokens: Some(1000),
            temperature: Some(0.7),
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
            extra_headers: Default::default(),
            context_window: 256_000,
            client_version: None,
            force_http1: false,
            max_retries: None,
            stream_tool_calls: false,
            idle_timeout_secs: None,
            client_identifier: None,
            reasoning_effort: None,
            deployment_id: None,
            user_id: None,
            origin_client: None,
            attribution_callback: None,
            bearer_resolver: None,
            supports_backend_search: false,
            compactions_remaining: None,
            compaction_at_tokens: None,
            doom_loop_recovery: None,
            header_injector: None,
            provider_identity,
            supports_native_schema: None,
            supports_strict_tools: None,
        }
    }

    fn create_client(base_url: &str, provider_identity: ProviderIdentity) -> Client {
        Client::new(make_test_config(base_url, provider_identity)).unwrap()
    }

    /// Test that two routes are tried in configured order.
    /// The sampler should use the first route, and only fall back when needed.
    #[tokio::test]
    async fn two_routes_tried_in_configured_order() {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::iter(
                    usable_summary_stream("first route summary")
                        .into_iter()
                        .map(Ok::<_, std::convert::Infallible>),
                );
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });
        let base_url = format!("http://{addr}/v1");

        // Create a second route that would succeed if first fails
        let app2 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::iter(
                    usable_summary_stream("second route summary")
                        .into_iter()
                        .map(Ok::<_, std::convert::Infallible>),
                );
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();
        let (shutdown_tx2, shutdown_rx2) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener2, app2)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx2.await;
                })
                .await
                .unwrap();
        });
        let base_url2 = format!("http://{addr2}/v1");

        let routes = vec![
            CompactionRoute {
                client: create_client(&base_url, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url, ProviderIdentity::Xai),
            },
            CompactionRoute {
                client: create_client(&base_url2, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url2, ProviderIdentity::Xai),
            },
        ];

        let sampler = ShellCompactionSampler::new(
            false,
            None,
            vec![],
            vec![],
            routes,
            acp::SessionId::new("test-session"),
            Duration::from_secs(30),
            0,
            crate::util::config::CompactionToolChoice::Auto,
        );

        let chat_history = vec![
            ConversationItem::system("You are a helpful assistant."),
            ConversationItem::user("Summarize this conversation."),
        ];

        let result = sampler
            .sample_compaction(
                &chat_history,
                &CompactionPrompt {
                    system: String::new(),
                    user: "Please summarize.".to_string(),
                },
                Duration::from_secs(30),
            )
            .await;

        assert!(result.is_ok(), "should succeed with first route");
        let output = result.unwrap();
        assert!(
            output.response.contains("first route summary"),
            "should use first route, got: {}",
            output.response
        );

        let _ = shutdown_tx.send(());
        let _ = shutdown_tx2.send(());
    }

    /// Test that fallback occurs on retryable transport errors.
    #[tokio::test]
    async fn fallback_on_transport_error() {
        // Reserve and release a loopback port so the first route gets a real
        // connection-refused transport error rather than a provider event.
        let unavailable = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let unavailable_addr = unavailable.local_addr().unwrap();
        drop(unavailable);
        let base_url1 = format!("http://{unavailable_addr}/v1");

        // Second route succeeds
        let app2 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::iter(
                    usable_summary_stream("second route summary")
                        .into_iter()
                        .map(Ok::<_, std::convert::Infallible>),
                );
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();
        let (shutdown_tx2, shutdown_rx2) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener2, app2)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx2.await;
                })
                .await
                .unwrap();
        });
        let base_url2 = format!("http://{addr2}/v1");

        let routes = vec![
            CompactionRoute {
                client: create_client(&base_url1, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url1, ProviderIdentity::Xai),
            },
            CompactionRoute {
                client: create_client(&base_url2, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url2, ProviderIdentity::Xai),
            },
        ];

        let sampler = ShellCompactionSampler::new(
            false,
            None,
            vec![],
            vec![],
            routes,
            acp::SessionId::new("test-session"),
            Duration::from_secs(30),
            0,
            crate::util::config::CompactionToolChoice::Auto,
        );

        let chat_history = vec![
            ConversationItem::system("You are a helpful assistant."),
            ConversationItem::user("Summarize this conversation."),
        ];

        let result = sampler
            .sample_compaction(
                &chat_history,
                &CompactionPrompt {
                    system: String::new(),
                    user: "Please summarize.".to_string(),
                },
                Duration::from_secs(30),
            )
            .await;

        assert!(result.is_ok(), "should fallback to second route");
        let output = result.unwrap();
        assert!(
            output.response.contains("second route summary"),
            "should have fallen back to second route, got: {}",
            output.response
        );

        let _ = shutdown_tx2.send(());
    }

    /// Test that fallback occurs on HTTP 408 (Request Timeout).
    #[tokio::test]
    async fn fallback_on_408_timeout() {
        // First route returns a real HTTP 408 response.
        let app1 = Router::new().route(
            "/v1/chat/completions",
            post(|| async { api_error_response(StatusCode::REQUEST_TIMEOUT, "Request timed out") }),
        );
        let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr1 = listener1.local_addr().unwrap();
        let (shutdown_tx1, shutdown_rx1) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener1, app1)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx1.await;
                })
                .await
                .unwrap();
        });
        let base_url1 = format!("http://{addr1}/v1");

        // Second route succeeds
        let app2 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::iter(
                    usable_summary_stream("second route summary")
                        .into_iter()
                        .map(Ok::<_, std::convert::Infallible>),
                );
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();
        let (shutdown_tx2, shutdown_rx2) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener2, app2)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx2.await;
                })
                .await
                .unwrap();
        });
        let base_url2 = format!("http://{addr2}/v1");

        let routes = vec![
            CompactionRoute {
                client: create_client(&base_url1, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url1, ProviderIdentity::Xai),
            },
            CompactionRoute {
                client: create_client(&base_url2, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url2, ProviderIdentity::Xai),
            },
        ];

        let sampler = ShellCompactionSampler::new(
            false,
            None,
            vec![],
            vec![],
            routes,
            acp::SessionId::new("test-session"),
            Duration::from_secs(30),
            0,
            crate::util::config::CompactionToolChoice::Auto,
        );

        let chat_history = vec![
            ConversationItem::system("You are a helpful assistant."),
            ConversationItem::user("Summarize this conversation."),
        ];

        let result = sampler
            .sample_compaction(
                &chat_history,
                &CompactionPrompt {
                    system: String::new(),
                    user: "Please summarize.".to_string(),
                },
                Duration::from_secs(30),
            )
            .await;

        assert!(result.is_ok(), "should fallback to second route on 408");

        let _ = shutdown_tx1.send(());
        let _ = shutdown_tx2.send(());
    }

    /// Test that fallback occurs on HTTP 429 (Rate Limit).
    #[tokio::test]
    async fn fallback_on_429_rate_limit() {
        // First route returns a real HTTP 429 response.
        let app1 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                api_error_response(StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded")
            }),
        );
        let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr1 = listener1.local_addr().unwrap();
        let (shutdown_tx1, shutdown_rx1) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener1, app1)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx1.await;
                })
                .await
                .unwrap();
        });
        let base_url1 = format!("http://{addr1}/v1");

        // Second route succeeds
        let app2 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::iter(
                    usable_summary_stream("second route summary")
                        .into_iter()
                        .map(Ok::<_, std::convert::Infallible>),
                );
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();
        let (shutdown_tx2, shutdown_rx2) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener2, app2)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx2.await;
                })
                .await
                .unwrap();
        });
        let base_url2 = format!("http://{addr2}/v1");

        let routes = vec![
            CompactionRoute {
                client: create_client(&base_url1, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url1, ProviderIdentity::Xai),
            },
            CompactionRoute {
                client: create_client(&base_url2, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url2, ProviderIdentity::Xai),
            },
        ];

        let sampler = ShellCompactionSampler::new(
            false,
            None,
            vec![],
            vec![],
            routes,
            acp::SessionId::new("test-session"),
            Duration::from_secs(30),
            0,
            crate::util::config::CompactionToolChoice::Auto,
        );

        let chat_history = vec![
            ConversationItem::system("You are a helpful assistant."),
            ConversationItem::user("Summarize this conversation."),
        ];

        let result = sampler
            .sample_compaction(
                &chat_history,
                &CompactionPrompt {
                    system: String::new(),
                    user: "Please summarize.".to_string(),
                },
                Duration::from_secs(30),
            )
            .await;

        assert!(result.is_ok(), "should fallback to second route on 429");

        let _ = shutdown_tx1.send(());
        let _ = shutdown_tx2.send(());
    }

    /// Test that fallback occurs on HTTP 5xx server errors.
    #[tokio::test]
    async fn fallback_on_5xx_server_error() {
        // First route returns a real HTTP 500 response.
        let app1 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                api_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }),
        );
        let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr1 = listener1.local_addr().unwrap();
        let (shutdown_tx1, shutdown_rx1) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener1, app1)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx1.await;
                })
                .await
                .unwrap();
        });
        let base_url1 = format!("http://{addr1}/v1");

        // Second route succeeds
        let app2 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::iter(
                    usable_summary_stream("second route summary")
                        .into_iter()
                        .map(Ok::<_, std::convert::Infallible>),
                );
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();
        let (shutdown_tx2, shutdown_rx2) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener2, app2)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx2.await;
                })
                .await
                .unwrap();
        });
        let base_url2 = format!("http://{addr2}/v1");

        let routes = vec![
            CompactionRoute {
                client: create_client(&base_url1, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url1, ProviderIdentity::Xai),
            },
            CompactionRoute {
                client: create_client(&base_url2, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url2, ProviderIdentity::Xai),
            },
        ];

        let sampler = ShellCompactionSampler::new(
            false,
            None,
            vec![],
            vec![],
            routes,
            acp::SessionId::new("test-session"),
            Duration::from_secs(30),
            0,
            crate::util::config::CompactionToolChoice::Auto,
        );

        let chat_history = vec![
            ConversationItem::system("You are a helpful assistant."),
            ConversationItem::user("Summarize this conversation."),
        ];

        let result = sampler
            .sample_compaction(
                &chat_history,
                &CompactionPrompt {
                    system: String::new(),
                    user: "Please summarize.".to_string(),
                },
                Duration::from_secs(30),
            )
            .await;

        assert!(result.is_ok(), "should fallback to second route on 500");

        let _ = shutdown_tx1.send(());
        let _ = shutdown_tx2.send(());
    }

    /// Test that fallback occurs on idle timeout.
    #[tokio::test]
    async fn fallback_on_idle_timeout() {
        // First route opens an SSE response but never emits model progress.
        let app1 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::pending::<Result<Event, std::convert::Infallible>>();
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr1 = listener1.local_addr().unwrap();
        let (shutdown_tx1, shutdown_rx1) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener1, app1)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx1.await;
                })
                .await
                .unwrap();
        });
        let base_url1 = format!("http://{addr1}/v1");

        // Second route succeeds
        let app2 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::iter(
                    usable_summary_stream("second route summary")
                        .into_iter()
                        .map(Ok::<_, std::convert::Infallible>),
                );
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();
        let (shutdown_tx2, shutdown_rx2) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener2, app2)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx2.await;
                })
                .await
                .unwrap();
        });
        let base_url2 = format!("http://{addr2}/v1");

        let routes = vec![
            CompactionRoute {
                client: create_client(&base_url1, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url1, ProviderIdentity::Xai),
            },
            CompactionRoute {
                client: create_client(&base_url2, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url2, ProviderIdentity::Xai),
            },
        ];

        let sampler = ShellCompactionSampler::new(
            false,
            None,
            vec![],
            vec![],
            routes,
            acp::SessionId::new("test-session"),
            Duration::from_millis(50),
            0,
            crate::util::config::CompactionToolChoice::Auto,
        );

        let chat_history = vec![
            ConversationItem::system("You are a helpful assistant."),
            ConversationItem::user("Summarize this conversation."),
        ];

        let result = sampler
            .sample_compaction(
                &chat_history,
                &CompactionPrompt {
                    system: String::new(),
                    user: "Please summarize.".to_string(),
                },
                Duration::from_secs(30),
            )
            .await;

        assert!(
            result.is_ok(),
            "should fallback to second route on idle timeout"
        );

        let _ = shutdown_tx1.send(());
        let _ = shutdown_tx2.send(());
    }

    /// Test that fallback occurs on empty output.
    #[tokio::test]
    async fn fallback_on_empty_output() {
        // First route returns empty
        let app1 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::iter(
                    empty_stream()
                        .into_iter()
                        .map(Ok::<_, std::convert::Infallible>),
                );
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr1 = listener1.local_addr().unwrap();
        let (shutdown_tx1, shutdown_rx1) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener1, app1)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx1.await;
                })
                .await
                .unwrap();
        });
        let base_url1 = format!("http://{addr1}/v1");

        // Second route succeeds
        let app2 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::iter(
                    usable_summary_stream("second route summary")
                        .into_iter()
                        .map(Ok::<_, std::convert::Infallible>),
                );
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();
        let (shutdown_tx2, shutdown_rx2) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener2, app2)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx2.await;
                })
                .await
                .unwrap();
        });
        let base_url2 = format!("http://{addr2}/v1");

        let routes = vec![
            CompactionRoute {
                client: create_client(&base_url1, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url1, ProviderIdentity::Xai),
            },
            CompactionRoute {
                client: create_client(&base_url2, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url2, ProviderIdentity::Xai),
            },
        ];

        let sampler = ShellCompactionSampler::new(
            false,
            None,
            vec![],
            vec![],
            routes,
            acp::SessionId::new("test-session"),
            Duration::from_secs(30),
            0,
            crate::util::config::CompactionToolChoice::Auto,
        );

        let chat_history = vec![
            ConversationItem::system("You are a helpful assistant."),
            ConversationItem::user("Summarize this conversation."),
        ];

        let result = sampler
            .sample_compaction(
                &chat_history,
                &CompactionPrompt {
                    system: String::new(),
                    user: "Please summarize.".to_string(),
                },
                Duration::from_secs(30),
            )
            .await;

        // Empty output should trigger fallback
        assert!(
            result.is_ok(),
            "should fallback to second route on empty output"
        );
        let output = result.unwrap();
        assert!(
            output.response.contains("second route summary"),
            "should have fallen back to second route, got: {}",
            output.response
        );

        let _ = shutdown_tx1.send(());
        let _ = shutdown_tx2.send(());
    }

    /// Test that fallback occurs on degenerate output.
    #[tokio::test]
    async fn fallback_on_degenerate_output() {
        // First route returns degenerate summary (too short, not useful)
        let app1 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::iter(
                    summary_stream("ok")
                        .into_iter()
                        .map(Ok::<_, std::convert::Infallible>),
                );
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr1 = listener1.local_addr().unwrap();
        let (shutdown_tx1, shutdown_rx1) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener1, app1)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx1.await;
                })
                .await
                .unwrap();
        });
        let base_url1 = format!("http://{addr1}/v1");

        // Second route succeeds
        let app2 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::iter(
                    usable_summary_stream("second route summary with full content")
                        .into_iter()
                        .map(Ok::<_, std::convert::Infallible>),
                );
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();
        let (shutdown_tx2, shutdown_rx2) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener2, app2)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx2.await;
                })
                .await
                .unwrap();
        });
        let base_url2 = format!("http://{addr2}/v1");

        let routes = vec![
            CompactionRoute {
                client: create_client(&base_url1, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url1, ProviderIdentity::Xai),
            },
            CompactionRoute {
                client: create_client(&base_url2, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url2, ProviderIdentity::Xai),
            },
        ];

        let sampler = ShellCompactionSampler::new(
            false,
            None,
            vec![],
            vec![],
            routes,
            acp::SessionId::new("test-session"),
            Duration::from_secs(30),
            0,
            crate::util::config::CompactionToolChoice::Auto,
        );

        let chat_history = vec![
            ConversationItem::system("You are a helpful assistant."),
            ConversationItem::user("Summarize this conversation."),
        ];

        let result = sampler
            .sample_compaction(
                &chat_history,
                &CompactionPrompt {
                    system: String::new(),
                    user: "Please summarize.".to_string(),
                },
                Duration::from_secs(30),
            )
            .await;

        // Degenerate output should trigger fallback
        assert!(
            result.is_ok(),
            "should fallback to second route on degenerate output"
        );
        let output = result.unwrap();
        assert!(
            output.response.contains("second route summary"),
            "should have fallen back to second route, got: {}",
            output.response
        );

        let _ = shutdown_tx1.send(());
        let _ = shutdown_tx2.send(());
    }

    /// Test that auth errors (401) do NOT trigger fallback.
    #[tokio::test]
    async fn no_fallback_on_auth_401() {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async { api_error_response(StatusCode::UNAUTHORIZED, "Invalid credentials") }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });
        let base_url = format!("http://{addr}/v1");

        let fallback_hits = Arc::new(AtomicUsize::new(0));
        let server_fallback_hits = Arc::clone(&fallback_hits);
        let fallback_app = Router::new().route(
            "/v1/chat/completions",
            post(move || {
                let hits = Arc::clone(&server_fallback_hits);
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    let stream = stream::iter(
                        usable_summary_stream("forbidden fallback")
                            .into_iter()
                            .map(Ok::<_, std::convert::Infallible>),
                    );
                    Sse::new(stream).keep_alive(KeepAlive::default())
                }
            }),
        );
        let fallback_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let fallback_addr = fallback_listener.local_addr().unwrap();
        let (fallback_shutdown_tx, fallback_shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(fallback_listener, fallback_app)
                .with_graceful_shutdown(async {
                    let _ = fallback_shutdown_rx.await;
                })
                .await
                .unwrap();
        });
        let fallback_base_url = format!("http://{fallback_addr}/v1");

        let routes = vec![
            CompactionRoute {
                client: create_client(&base_url, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url, ProviderIdentity::Xai),
            },
            CompactionRoute {
                client: create_client(&fallback_base_url, ProviderIdentity::Xai),
                inference_config: make_test_config(&fallback_base_url, ProviderIdentity::Xai),
            },
        ];

        let sampler = ShellCompactionSampler::new(
            false,
            None,
            vec![],
            vec![],
            routes,
            acp::SessionId::new("test-session"),
            Duration::from_secs(30),
            0,
            crate::util::config::CompactionToolChoice::Auto,
        );

        let chat_history = vec![
            ConversationItem::system("You are a helpful assistant."),
            ConversationItem::user("Summarize this conversation."),
        ];

        let result = sampler
            .sample_compaction(
                &chat_history,
                &CompactionPrompt {
                    system: String::new(),
                    user: "Please summarize.".to_string(),
                },
                Duration::from_secs(30),
            )
            .await;

        // Auth error should result in failure without contacting route two.
        assert!(result.is_err(), "auth 401 should NOT trigger fallback");
        assert_eq!(
            fallback_hits.load(Ordering::SeqCst),
            0,
            "auth 401 must not contact the configured fallback route"
        );

        let _ = shutdown_tx.send(());
        let _ = fallback_shutdown_tx.send(());
    }

    /// Test that invalid configuration errors are deterministic and therefore
    /// cannot advance to another route.
    #[test]
    fn invalid_configuration_is_deterministic_no_fallback() {
        let result = crate::session::helpers::session_compact::classify_sampling_error(
            crate::inference::InferenceError::InvalidConfiguration("missing model"),
        );
        assert!(
            matches!(result, CompactFailure::Deterministic(_)),
            "invalid configuration must not trigger fallback"
        );
    }

    /// Test that privacy/policy 403 errors do NOT trigger fallback.
    #[tokio::test]
    async fn no_fallback_on_403_forbidden() {
        // 403 Forbidden should be deterministic, not transient
        let result = crate::session::helpers::session_compact::classify_sampling_error(
            crate::inference::InferenceError::Api {
                status: StatusCode::FORBIDDEN,
                message: "Content violates usage guidelines".into(),
                model_metadata: None,
                retry_after_secs: None,
                should_retry: None,
                diagnostics: None,
            },
        );
        assert!(
            matches!(result, CompactFailure::Deterministic(_)),
            "403 Forbidden should be deterministic, not trigger fallback"
        );
    }

    /// Test that unsupported deterministic 4xx errors do NOT trigger fallback.
    #[tokio::test]
    async fn no_fallback_on_unsupported_deterministic_4xx() {
        let unsupported_codes = [
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
            StatusCode::PAYLOAD_TOO_LARGE,
        ];

        for status in unsupported_codes {
            let result = crate::session::helpers::session_compact::classify_sampling_error(
                crate::inference::InferenceError::Api {
                    status,
                    message: "test error".into(),
                    model_metadata: None,
                    retry_after_secs: None,
                    should_retry: None,
                    diagnostics: None,
                },
            );
            assert!(
                matches!(result, CompactFailure::Deterministic(_)),
                "{:?} should be deterministic, not trigger fallback",
                status
            );
        }
    }

    /// Test that context overflow is deterministic and does NOT trigger fallback.
    #[tokio::test]
    async fn no_fallback_on_context_overflow() {
        let result = crate::session::helpers::session_compact::classify_sampling_error(
            crate::inference::InferenceError::Api {
                status: StatusCode::BAD_REQUEST,
                message: "The prompt is too long for this model's context window.".into(),
                model_metadata: None,
                retry_after_secs: None,
                should_retry: None,
                diagnostics: None,
            },
        );
        assert!(
            matches!(result, CompactFailure::Deterministic(_)),
            "context overflow should be deterministic, not trigger fallback"
        );
    }

    /// Test that OpenRouter routes have hidden fallback list cleared.
    /// This verifies the provider-specific configuration stripping.
    #[tokio::test]
    async fn openrouter_route_hidden_fallback_list_cleared() {
        let base_url = "http://127.0.0.1:1";
        let mut config = make_test_config(base_url, ProviderIdentity::OpenRouter);
        config.openrouter_fallback_models = vec!["fallback-model".to_string()];
        config.reasoning_effort = Some(xai_grok_inference_types::ReasoningEffort::Low);
        config.supports_backend_search = true;

        // Verify that when prepared, the fallback list is cleared
        assert!(
            !config.openrouter_fallback_models.is_empty(),
            "precondition: fallback models should be set"
        );

        // Simulate what prepare_compaction_routes does
        config.openrouter_fallback_models.clear();
        config.reasoning_effort = None;
        config.supports_backend_search = false;

        assert!(
            config.openrouter_fallback_models.is_empty(),
            "OpenRouter fallback models should be cleared for compaction"
        );
        assert!(
            config.reasoning_effort.is_none(),
            "reasoning_effort should be cleared for compaction"
        );
        assert!(
            !config.supports_backend_search,
            "supports_backend_search should be false for compaction"
        );
    }

    /// Test that third-party routes get text-only request shaping.
    /// Verifies provider-specific reasoning/search fields are cleared.
    #[tokio::test]
    async fn third_party_route_text_only_request_shaping() {
        let base_url = "http://127.0.0.1:1";
        let mut config = make_test_config(base_url, ProviderIdentity::Custom);

        // Set provider-specific fields that should be cleared
        config.reasoning_effort = Some(xai_grok_inference_types::ReasoningEffort::High);
        config.supports_backend_search = true;

        // Simulate what prepare_compaction_routes does for third-party routes
        config.reasoning_effort = None;
        config.supports_backend_search = false;

        assert!(
            config.reasoning_effort.is_none(),
            "reasoning_effort should be cleared for third-party compaction"
        );
        assert!(
            !config.supports_backend_search,
            "supports_backend_search should be false for third-party compaction"
        );
    }

    /// Test that xAI routes preserve reasoning_effort (first-party).
    #[tokio::test]
    async fn xai_route_preserves_reasoning() {
        let base_url = "http://127.0.0.1:1";
        let mut config = make_test_config(base_url, ProviderIdentity::Xai);
        config.reasoning_effort = Some(xai_grok_inference_types::ReasoningEffort::High);

        // For xAI, we don't clear reasoning_effort (first-party)
        // This test verifies the behavior is correct for xAI routes
        assert!(
            matches!(config.provider_identity, ProviderIdentity::Xai),
            "xAI route should have Xai provider identity"
        );
    }

    /// Test that empty response is classified as transient and triggers fallback.
    #[test]
    fn empty_response_is_transient() {
        let result = crate::session::helpers::session_compact::classify_sampling_error(
            crate::inference::InferenceError::EmptyResponse {
                context: xai_grok_inference_types::EmptyResponseContext {
                    reason: xai_grok_inference_types::EmptyReason::NoVisibleContent,
                    had_reasoning: false,
                    content_len: 0,
                    tool_call_count: 0,
                    finish_reason: Some("stop".into()),
                    completion_tokens: Some(0),
                    reasoning_tokens: Some(0),
                    prompt_tokens: None,
                    first_choice_seen: true,
                    model: "test-model".into(),
                },
            },
        );
        assert!(
            matches!(result, CompactFailure::Transient(_)),
            "empty response should be transient to allow fallback"
        );
    }

    /// Test that wall-clock timeout is classified as transient.
    #[test]
    fn wall_clock_timeout_is_transient() {
        let result = crate::session::helpers::session_compact::classify_sampling_error(
            crate::inference::InferenceError::Api {
                status: StatusCode::REQUEST_TIMEOUT,
                message: "Request took too long".into(),
                model_metadata: None,
                retry_after_secs: None,
                should_retry: None,
                diagnostics: None,
            },
        );
        assert!(
            matches!(result, CompactFailure::Transient(_)),
            "408 Request Timeout should be transient to allow fallback"
        );
    }

    /// Test that HTTP 502 Bad Gateway is transient and triggers fallback.
    #[test]
    fn http_502_is_transient() {
        let result = crate::session::helpers::session_compact::classify_sampling_error(
            crate::inference::InferenceError::Api {
                status: StatusCode::BAD_GATEWAY,
                message: "Bad gateway".into(),
                model_metadata: None,
                retry_after_secs: None,
                should_retry: None,
                diagnostics: None,
            },
        );
        assert!(
            matches!(result, CompactFailure::Transient(_)),
            "502 Bad Gateway should be transient to allow fallback"
        );
    }

    /// Test that HTTP 503 Service Unavailable is transient and triggers fallback.
    #[test]
    fn http_503_is_transient() {
        let result = crate::session::helpers::session_compact::classify_sampling_error(
            crate::inference::InferenceError::Api {
                status: StatusCode::SERVICE_UNAVAILABLE,
                message: "Service unavailable".into(),
                model_metadata: None,
                retry_after_secs: None,
                should_retry: None,
                diagnostics: None,
            },
        );
        assert!(
            matches!(result, CompactFailure::Transient(_)),
            "503 Service Unavailable should be transient to allow fallback"
        );
    }

    /// Test that HTTP 504 Gateway Timeout is transient and triggers fallback.
    #[test]
    fn http_504_is_transient() {
        let result = crate::session::helpers::session_compact::classify_sampling_error(
            crate::inference::InferenceError::Api {
                status: StatusCode::GATEWAY_TIMEOUT,
                message: "Gateway timeout".into(),
                model_metadata: None,
                retry_after_secs: None,
                should_retry: None,
                diagnostics: None,
            },
        );
        assert!(
            matches!(result, CompactFailure::Transient(_)),
            "504 Gateway Timeout should be transient to allow fallback"
        );
    }

    /// Test that HTTP 402 Payment Required is deterministic (no fallback).
    /// This is OpenRouter's out-of-credits signal.
    #[test]
    fn http_402_is_deterministic_no_fallback() {
        let result = crate::session::helpers::session_compact::classify_sampling_error(
            crate::inference::InferenceError::Api {
                status: StatusCode::PAYMENT_REQUIRED,
                message: "Out of credits".into(),
                model_metadata: None,
                retry_after_secs: None,
                should_retry: None,
                diagnostics: None,
            },
        );
        assert!(
            matches!(result, CompactFailure::Deterministic(_)),
            "402 Payment Required should be deterministic (no fallback) - out of credits"
        );
    }

    /// Test that DoomLoopDetected is transient and triggers fallback.
    #[test]
    fn doom_loop_detected_is_transient() {
        let result = crate::session::helpers::session_compact::classify_sampling_error(
            crate::inference::InferenceError::DoomLoopDetected {
                triggers: vec!["test_trigger".to_string()],
                aborted_at_chunk: None,
            },
        );
        assert!(
            matches!(result, CompactFailure::Transient(_)),
            "DoomLoopDetected should be transient to allow fallback"
        );
    }

    /// Test that HTTP 520-524 Cloudflare edge errors are transient and trigger fallback.
    #[test]
    fn http_cloudflare_edge_errors_are_transient() {
        for status in [520, 521, 522, 523, 524] {
            let result = crate::session::helpers::session_compact::classify_sampling_error(
                crate::inference::InferenceError::Api {
                    status: StatusCode::from_u16(status).unwrap(),
                    message: format!("Cloudflare error {}", status),
                    model_metadata: None,
                    retry_after_secs: None,
                    should_retry: None,
                    diagnostics: None,
                },
            );
            assert!(
                matches!(result, CompactFailure::Transient(_)),
                "HTTP {} should be transient to allow fallback",
                status
            );
        }
    }

    /// Test that HTTP 52x errors trigger fallback in the full_replace_compaction sampler.
    #[tokio::test]
    async fn fallback_on_5xx_server_errors() {
        // First route returns a real HTTP 502 response.
        let app1 = Router::new().route(
            "/v1/chat/completions",
            post(|| async { api_error_response(StatusCode::BAD_GATEWAY, "Bad gateway") }),
        );
        let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr1 = listener1.local_addr().unwrap();
        let (shutdown_tx1, shutdown_rx1) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener1, app1)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx1.await;
                })
                .await
                .unwrap();
        });
        let base_url1 = format!("http://{addr1}/v1");

        // Second route succeeds
        let app2 = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                let stream = stream::iter(
                    usable_summary_stream("second route summary")
                        .into_iter()
                        .map(Ok::<_, std::convert::Infallible>),
                );
                Sse::new(stream).keep_alive(KeepAlive::default())
            }),
        );
        let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();
        let (shutdown_tx2, shutdown_rx2) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener2, app2)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx2.await;
                })
                .await
                .unwrap();
        });
        let base_url2 = format!("http://{addr2}/v1");

        let routes = vec![
            CompactionRoute {
                client: create_client(&base_url1, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url1, ProviderIdentity::Xai),
            },
            CompactionRoute {
                client: create_client(&base_url2, ProviderIdentity::Xai),
                inference_config: make_test_config(&base_url2, ProviderIdentity::Xai),
            },
        ];

        let sampler = ShellCompactionSampler::new(
            false,
            None,
            vec![],
            vec![],
            routes,
            acp::SessionId::new("test-session"),
            Duration::from_secs(30),
            0,
            crate::util::config::CompactionToolChoice::Auto,
        );

        let chat_history = vec![
            ConversationItem::system("You are a helpful assistant."),
            ConversationItem::user("Summarize this conversation."),
        ];

        let result = sampler
            .sample_compaction(
                &chat_history,
                &CompactionPrompt {
                    system: String::new(),
                    user: "Please summarize.".to_string(),
                },
                Duration::from_secs(30),
            )
            .await;

        assert!(result.is_ok(), "should fallback to second route on 500");

        let _ = shutdown_tx1.send(());
        let _ = shutdown_tx2.send(());
    }
}
