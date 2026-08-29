//! Read-only tool resolution for the compaction summarizer.
//!
//! Compaction replaces hundreds of turns with one summary, and the history it
//! summarizes references artifacts the summarizer cannot otherwise read: a
//! screenshot the user attached, a background-task log spilled to a file, a
//! transcript segment from an earlier compaction. This module lets the
//! compaction model open those artifacts itself, on its own initiative, before
//! it writes the summary.
//!
//! Two deliberate boundaries:
//!
//! 1. **Read-only allowlist.** Compaction can look, never mutate. Anything the
//!    model asks for outside [`COMPACTION_TOOL_ALLOWLIST`] is answered with an
//!    error result instead of being executed.
//! 2. **Text-only handoff to the summary round.** Exploration rounds do use
//!    native tool calls, but their results are folded into one synthetic user
//!    block rather than replayed as `assistant`/`tool_result` item pairs. The
//!    final summary request then advertises no tools at all, which keeps the
//!    cross-provider property the sampler relies on: a tool-trained model
//!    cannot answer a summarization request with a tool call, and no provider
//!    sees a `tool_result` whose definition it was never given.
//!
//! Resolution is best-effort by construction: every failure path (bad
//! arguments, a tool error, an unparseable response, cancellation) abandons
//! exploration and lets the summary proceed from the history alone. A broken
//! resolver must never turn into a failed compaction.

use std::sync::Arc;

use async_trait::async_trait;
use xai_grok_inference::InferenceConfig;
use xai_grok_inference_types::{ConversationItem, ToolCall, ToolSpec};

use crate::inference::{Client as OaiCompatClient, ConversationRequest, ConversationToolChoice};

/// Tool names the compaction summarizer may call: read-only lookups that can
/// resolve a file, a search hit, or a directory referenced in history.
pub const COMPACTION_TOOL_ALLOWLIST: &[&str] = &["read_file", "grep", "list_dir"];

/// Extra summarizer calls one compaction may spend on resolution. Each round is
/// one full sampler request, and compactions already run for minutes.
const MAX_RESOLVER_ROUNDS: u32 = 2;

/// Tool calls executed across all rounds. Bounds the worst case where a model
/// keeps asking instead of summarizing.
const MAX_RESOLVER_CALLS: u32 = 8;

/// Per-call cap on what the summarizer gets to read back.
const MAX_RESOLVER_RESULT_CHARS: usize = 8_000;

/// Cap on the folded block, so resolution cannot inflate the summary request
/// past the budget the input ladder was sized for.
const MAX_RESOLVER_CONTEXT_CHARS: usize = 24_000;

/// Executes one allowlisted tool call on behalf of the summarizer.
#[async_trait]
pub trait CompactionToolResolver: Send + Sync {
    /// Tool definitions to advertise. Already intersected with what this session
    /// actually registers, so an empty vec means "nothing to resolve with".
    fn specs(&self) -> Vec<ToolSpec>;

    /// Run one call. `Ok` text is shown to the summarizer; `Err` is shown as a
    /// tool error result and never fails the compaction.
    async fn resolve(&self, name: &str, args: serde_json::Value) -> Result<String, String>;
}

/// Resolver backed by the session's workspace toolset.
pub struct WorkspaceCompactionResolver {
    workspace_ops: xai_grok_workspace::WorkspaceOps,
    session_id: String,
    specs: Vec<ToolSpec>,
}

impl WorkspaceCompactionResolver {
    /// Keep only the allowlisted tools this session actually advertises, so the
    /// summarizer is never offered a tool the registry would reject.
    pub fn new(
        workspace_ops: xai_grok_workspace::WorkspaceOps,
        session_id: String,
        candidate_specs: &[ToolSpec],
    ) -> Self {
        let specs = candidate_specs
            .iter()
            .filter(|spec| COMPACTION_TOOL_ALLOWLIST.contains(&spec.name.as_str()))
            .cloned()
            .collect();
        Self {
            workspace_ops,
            session_id,
            specs,
        }
    }
}

#[async_trait]
impl CompactionToolResolver for WorkspaceCompactionResolver {
    fn specs(&self) -> Vec<ToolSpec> {
        self.specs.clone()
    }

    async fn resolve(&self, name: &str, args: serde_json::Value) -> Result<String, String> {
        let call_id = uuid::Uuid::new_v4().to_string();
        self.workspace_ops
            .call_tool(name, args, &call_id, Some(self.session_id.as_str()))
            .await
            .map(|result| result.prompt_text)
            .map_err(|error| error.to_string())
    }
}

/// Outcome of the resolution phase.
#[derive(Debug, Default)]
pub struct ResolverOutcome {
    /// The folded lookup block, `None` when nothing was resolved.
    pub lookups: Option<String>,
    /// Summarizer calls spent on resolution (0 when nothing ran).
    pub rounds: u32,
    /// Tool calls executed.
    pub calls: u32,
    /// The summarizer answered with prose instead of asking for anything; use
    /// it as the summary and skip the streaming round.
    pub early_summary: Option<String>,
}

/// Run the read-only resolution rounds over `turns` and return the folded block.
///
/// `turns` is the conversation **without** the summarization prompt: that prompt
/// tells the model not to call tools, so resolution has to happen before it is
/// appended. The caller adds [`ResolverOutcome::lookups`] to the conversation and
/// then builds the summarization prompt on top of it.
///
/// `client` is the same compaction route that will produce the summary, so
/// resolution never discloses history to a different provider.
pub async fn run_resolver_rounds(
    client: &OaiCompatClient,
    inference_config: &InferenceConfig,
    session_id: &str,
    turns: &[ConversationItem],
    resolver: &dyn CompactionToolResolver,
    cancel: &tokio_util::sync::CancellationToken,
) -> ResolverOutcome {
    let mut outcome = ResolverOutcome::default();
    let specs = resolver.specs();
    if specs.is_empty() {
        return outcome;
    }

    let mut lookups: Vec<String> = Vec::new();
    let mut context_chars: usize = 0;
    // Tell the model what it may look up, so a bare tool name never reaches it
    // without an instruction on how to use it.
    let mut working_history: Vec<ConversationItem> = turns.to_vec();
    working_history.push(ConversationItem::user(resolver_instructions()));

    for _round in 0..MAX_RESOLVER_ROUNDS {
        if cancel.is_cancelled() || outcome.calls >= MAX_RESOLVER_CALLS {
            break;
        }
        let request = ConversationRequest {
            items: working_history.clone(),
            tool_choice: Some(ConversationToolChoice::Auto),
            tools: specs.clone(),
            hosted_tools: Vec::new(),
            model: Some(inference_config.model.to_owned()),
            temperature: None,
            x_grok_conv_id: Some(session_id.to_owned()),
            x_grok_req_id: Some(resolver_request_id()),
            x_grok_session_id: Some(session_id.to_owned()),
            x_grok_agent_id: Some(xai_grok_telemetry::id::agent_id()),
            ..Default::default()
        };
        let response = match client.conversation_collect(request).await {
            Ok(response) => response,
            Err(error) => {
                // Best-effort: an unresolvable round is exactly the pre-change
                // behavior, so keep the summary rather than fail the compact.
                tracing::warn!(
                    model = %inference_config.model,
                    error = %error,
                    "compaction tool resolution abandoned; summarizing without it"
                );
                break;
            }
        };
        outcome.rounds += 1;

        let Some((tool_calls, answer_text)) =
            response.items.iter().rev().find_map(|item| match item {
                ConversationItem::Assistant(assistant) => {
                    Some((assistant.tool_calls.clone(), assistant.content.to_string()))
                }
                _ => None,
            })
        else {
            break;
        };
        if tool_calls.is_empty() {
            // The summarizer chose to answer rather than look anything up.
            if !answer_text.trim().is_empty() {
                outcome.early_summary = Some(answer_text);
            }
            break;
        }

        // Every advertised tool call needs a matching result item, or the next
        // round is a conversation with a dangling call.
        let mut round_items: Vec<ConversationItem> = Vec::new();
        round_items.extend(response.items.into_iter().filter(|item| {
            matches!(
                item,
                ConversationItem::Assistant(_) | ConversationItem::ToolResult(_)
            )
        }));
        for call in tool_calls.iter() {
            let rendered = execute_lookup(resolver, call, cancel).await;
            outcome.calls += 1;
            let kept = if context_chars + rendered.len() > MAX_RESOLVER_CONTEXT_CHARS {
                let room = MAX_RESOLVER_CONTEXT_CHARS.saturating_sub(context_chars);
                context_chars = MAX_RESOLVER_CONTEXT_CHARS;
                truncate_chars(&rendered, room)
            } else {
                context_chars += rendered.len();
                rendered
            };
            lookups.push(kept.clone());
            round_items.push(ConversationItem::tool_result(call.id.as_ref(), kept));
            if outcome.calls >= MAX_RESOLVER_CALLS || context_chars >= MAX_RESOLVER_CONTEXT_CHARS {
                break;
            }
        }
        // Results feed the next exploration round natively; the summary round
        // never sees these items (see the module docs).
        working_history.extend(round_items);

        if context_chars >= MAX_RESOLVER_CONTEXT_CHARS {
            break;
        }
    }

    if outcome.early_summary.is_none() && !lookups.is_empty() {
        outcome.lookups = Some(format_compacted_lookups(&lookups));
    }
    outcome
}

/// Execute one requested call and render it for the folded block.
async fn execute_lookup(
    resolver: &dyn CompactionToolResolver,
    call: &ToolCall,
    cancel: &tokio_util::sync::CancellationToken,
) -> String {
    let header = format!(
        "<compaction_lookup tool=\"{}\" args={}>",
        call.name, call.arguments
    );
    if cancel.is_cancelled() {
        return format!("{header}\n[cancelled before execution]\n</compaction_lookup>");
    }
    let args: serde_json::Value = match serde_json::from_str(&call.arguments) {
        Ok(args) => args,
        Err(error) => {
            return format!("{header}\n[invalid tool arguments: {error}]\n</compaction_lookup>",);
        }
    };
    if !COMPACTION_TOOL_ALLOWLIST.contains(&call.name.as_str()) {
        return format!(
            "{header}\n[tool unavailable during compaction; allowed: {COMPACTION_TOOL_ALLOWLIST:?}]\n</compaction_lookup>"
        );
    }
    let body = match resolver.resolve(&call.name, args).await {
        Ok(text) => truncate_chars(&text, MAX_RESOLVER_RESULT_CHARS),
        Err(error) => format!("[tool error: {error}]"),
    };
    format!("{header}\n{body}\n</compaction_lookup>")
}

/// Instructions that open the resolution phase. Kept separate from the
/// summarization prompt so the summary contract stays byte-identical.
fn resolver_instructions() -> String {
    "You may resolve artifacts referenced in the conversation before writing \
     the summary. Call `read_file`, `grep`, or `list_dir` on any file, log, or \
     segment path above that you need in order to describe accurately what was \
     changed, seen, or decided. Ask only for what the summary is missing; if you \
     already have enough, reply with the summary text alone and no tool call."
        .to_string()
}

/// Fold executed lookups into the single user block the summary round receives.
fn format_compacted_lookups(lookups: &[String]) -> String {
    format!(
        "<compaction_context_lookups>\nRead-only lookups resolved by the \
         compaction step, newest last. Treat these as ground truth for files, \
         logs, and segments referenced in the conversation above.\n\n{}\n\
         </compaction_context_lookups>",
        lookups.join("\n")
    )
}

/// Request id for a resolution round. Distinct from the summary round's id so
/// provider-side logs separate lookups from the summarization call itself.
fn resolver_request_id() -> String {
    format!("xai-compact-lookup-{}", uuid::Uuid::new_v4())
}

/// Char-boundary-safe truncation; compaction text is model-facing, never parsed.
fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut kept: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    kept.push('…');
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(name: &str) -> ToolSpec {
        ToolSpec {
            name: name.to_owned(),
            description: None,
            parameters: serde_json::json!({ "type": "object" }),
            strict: None,
        }
    }

    /// An allowlisted spec survives, everything else is dropped, so the
    /// summarizer is never offered a tool the session would refuse to run.
    #[test]
    fn resolver_advertises_only_allowlisted_read_only_tools() {
        let specs = vec![
            spec("read_file"),
            spec("grep"),
            spec("list_dir"),
            spec("search_replace"),
            spec("run_terminal_command"),
        ];
        let kept = specs
            .iter()
            .filter(|spec| COMPACTION_TOOL_ALLOWLIST.contains(&spec.name.as_str()))
            .map(|spec| spec.name.clone())
            .collect::<Vec<_>>();
        assert_eq!(kept, vec!["read_file", "grep", "list_dir"]);
    }

    /// A call outside the allowlist is answered, not executed: the summarizer
    /// gets an explanatory error result and compaction keeps going.
    #[tokio::test]
    async fn non_allowlisted_call_is_answered_without_execution() {
        struct Recorder;
        #[async_trait]
        impl CompactionToolResolver for Recorder {
            fn specs(&self) -> Vec<ToolSpec> {
                vec![]
            }
            async fn resolve(
                &self,
                _name: &str,
                _args: serde_json::Value,
            ) -> Result<String, String> {
                unreachable!("only allowlisted tools may execute")
            }
        }
        let cancel = tokio_util::sync::CancellationToken::new();
        let rendered = execute_lookup(
            &Recorder,
            &ToolCall {
                id: Arc::from("call-1"),
                name: "search_replace".into(),
                arguments: Arc::from(r#"{"file_path":"a.rs"}"#),
            },
            &cancel,
        )
        .await;
        assert!(rendered.contains("tool unavailable during compaction"));
    }

    /// Malformed tool arguments from a flash-class summarizer must degrade to a
    /// readable result rather than an execution attempt.
    #[tokio::test]
    async fn malformed_arguments_are_reported_not_executed() {
        struct Recorder;
        #[async_trait]
        impl CompactionToolResolver for Recorder {
            fn specs(&self) -> Vec<ToolSpec> {
                vec![]
            }
            async fn resolve(
                &self,
                _name: &str,
                _args: serde_json::Value,
            ) -> Result<String, String> {
                unreachable!("arguments never parsed successfully")
            }
        }
        let cancel = tokio_util::sync::CancellationToken::new();
        let rendered = execute_lookup(
            &Recorder,
            &ToolCall {
                id: Arc::from("call-2"),
                name: "read_file".into(),
                arguments: Arc::from("{not json"),
            },
            &cancel,
        )
        .await;
        assert!(rendered.contains("invalid tool arguments"));
    }

    /// Tool output is bounded per call so one `read_file` cannot inflate the
    /// summary request past its input budget.
    #[test]
    fn lookup_results_are_char_bounded() {
        let long = "x".repeat(MAX_RESOLVER_RESULT_CHARS + 500);
        let cut = truncate_chars(&long, MAX_RESOLVER_RESULT_CHARS);
        assert_eq!(cut.chars().count(), MAX_RESOLVER_RESULT_CHARS);
        assert!(cut.ends_with('…'));
        assert_eq!(truncate_chars("short", 100), "short");
    }

    /// The folded block is a plain user message: no native tool pairing survives
    /// into the summary request, which is what keeps third-party providers from
    /// seeing a `tool_result` they were never given a definition for.
    #[test]
    fn folded_lookup_block_is_plain_text() {
        let block = format_compacted_lookups(&[
            "<compaction_lookup tool=\"grep\">hits</compaction_lookup>".to_string(),
        ]);
        assert!(block.starts_with("<compaction_context_lookups>"));
        assert!(block.contains("grep"));
        // The handoff item is a plain user message, not an assistant turn with
        // native tool calls that a third-party provider could not pair up.
        let item = ConversationItem::user(block);
        match item {
            ConversationItem::User(_) => {}
            other => panic!("folded block must stay a plain user item, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod resolver_round_tests {
    use super::*;
    use crate::inference::{ApiBackend, Client, InferenceConfig};
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::response::sse::{Event, KeepAlive, Sse};
    use axum::routing::post;
    use axum::{Json, Router};
    use futures_util::stream;
    use serde_json::json;
    use std::convert::Infallible;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex as AsyncMutex;
    use xai_grok_inference::config::ProviderIdentity;

    #[derive(Clone)]
    struct ServerState {
        count: Arc<AtomicUsize>,
        bodies: Arc<AsyncMutex<Vec<serde_json::Value>>>,
        /// When set, every round answers with a tool call, so the round budget
        /// ends resolution rather than the model finishing.
        always_call_tools: bool,
    }

    fn tool_call_event() -> Event {
        Event::default().data(
            json!({
                "id": "chatcmpl-lookup",
                "object": "chat.completion.chunk",
                "created": 1,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "role": "assistant",
                        "tool_calls": [{
                            "index": 0,
                            "id": "call-1",
                            "type": "function",
                            "function": {
                                "name": "read_file",
                                "arguments": "{\"target_file\":\"segment_000.md\"}"
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }]
            })
            .to_string(),
        )
    }

    fn text_event(text: &str) -> Event {
        Event::default().data(
            json!({
                "id": "chatcmpl-summary",
                "object": "chat.completion.chunk",
                "created": 1,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "delta": { "role": "assistant", "content": text },
                    "finish_reason": "stop"
                }]
            })
            .to_string(),
        )
    }

    async fn handle(State(state): State<ServerState>, body: String) -> impl IntoResponse {
        let n = state.count.fetch_add(1, Ordering::SeqCst);
        state
            .bodies
            .lock()
            .await
            .push(serde_json::from_str(&body).unwrap_or(json!({"raw": body})));
        let events = if state.always_call_tools || n == 0 {
            vec![tool_call_event()]
        } else {
            vec![text_event("<summary>resolved summary</summary>")]
        }
        .into_iter()
        .chain(std::iter::once(Event::default().data("[DONE]")))
        .map(Ok::<_, Infallible>);
        Sse::new(stream::iter(events)).keep_alive(KeepAlive::default())
    }

    async fn serve(always_call_tools: bool) -> (String, ServerState) {
        let state = ServerState {
            count: Arc::new(AtomicUsize::new(0)),
            bodies: Arc::new(AsyncMutex::new(Vec::new())),
            always_call_tools,
        };
        let app = Router::new()
            .route("/v1/chat/completions", post(handle))
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}/v1"), state)
    }

    async fn failing_server() -> String {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": {"message": "upstream exploded"}})),
                )
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/v1")
    }

    /// Route config and client for one fake provider, sharing one base url.
    fn route(base_url: &str) -> (Client, InferenceConfig) {
        let config = InferenceConfig {
            api_key: Some("test-key".to_string()),
            base_url: base_url.to_string(),
            model: "test-model".to_string(),
            api_backend: ApiBackend::ChatCompletions,
            provider_identity: ProviderIdentity::Custom,
            ..Default::default()
        };
        let client = Client::new(config.clone()).expect("test client");
        (client, config)
    }

    struct FakeResolver {
        executed: Arc<AsyncMutex<Vec<(String, serde_json::Value)>>>,
    }

    #[async_trait]
    impl CompactionToolResolver for FakeResolver {
        fn specs(&self) -> Vec<ToolSpec> {
            vec![ToolSpec {
                name: "read_file".to_owned(),
                description: None,
                parameters: serde_json::json!({ "type": "object" }),
                strict: None,
            }]
        }
        async fn resolve(&self, name: &str, args: serde_json::Value) -> Result<String, String> {
            self.executed.lock().await.push((name.to_owned(), args));
            Ok("THE FILE SAID THIS".to_string())
        }
    }

    fn turns() -> Vec<ConversationItem> {
        vec![ConversationItem::user("please summarize this work")]
    }

    /// The summarizer's tool call is executed read-only and its result is fed
    /// back, so the model writes the summary from real file contents instead of
    /// its memory of them.
    #[tokio::test]
    async fn lookup_round_executes_tool_and_feeds_result_back() {
        let (base_url, state) = serve(false).await;
        let executed = Arc::new(AsyncMutex::new(Vec::new()));
        let cancel = tokio_util::sync::CancellationToken::new();
        let (client, config) = route(&base_url);

        let outcome = run_resolver_rounds(
            &client,
            &config,
            "session-1",
            &turns(),
            &FakeResolver {
                executed: executed.clone(),
            },
            &cancel,
        )
        .await;

        assert_eq!(outcome.rounds, 2, "one lookup round, one answering round");
        assert_eq!(outcome.calls, 1);
        assert_eq!(
            executed.lock().await.as_slice(),
            &[(
                "read_file".to_string(),
                json!({"target_file": "segment_000.md"})
            )][..]
        );
        // The answering round must carry the executed tool result.
        let bodies = state.bodies.lock().await;
        let second = bodies
            .get(1)
            .expect("a second round request reached the provider");
        let rendered = second.to_string();
        assert!(
            rendered.contains("THE FILE SAID THIS"),
            "tool result never reached the summarizer: {rendered}"
        );
        // Answering in-round means no separate streaming summary call.
        assert_eq!(
            outcome.early_summary.as_deref(),
            Some("<summary>resolved summary</summary>")
        );
        assert!(outcome.lookups.is_none());
    }

    /// When the model keeps asking, resolution stops on budget and the results
    /// travel to the summary round as one plain-text block. No native tool
    /// pairing survives, which is what keeps third-party providers from
    /// rejecting a summary request that advertises no tools.
    #[tokio::test]
    async fn round_budget_exhaustion_folds_lookups_into_plain_text() {
        let (base_url, _state) = serve(true).await;
        let executed = Arc::new(AsyncMutex::new(Vec::new()));
        let cancel = tokio_util::sync::CancellationToken::new();
        let (client, config) = route(&base_url);

        let outcome = run_resolver_rounds(
            &client,
            &config,
            "session-1",
            &turns(),
            &FakeResolver {
                executed: executed.clone(),
            },
            &cancel,
        )
        .await;

        assert_eq!(outcome.rounds, MAX_RESOLVER_ROUNDS);
        assert!(outcome.early_summary.is_none());
        let lookups = outcome.lookups.expect("lookups folded for summary");
        assert!(lookups.starts_with("<compaction_context_lookups>"));
        assert!(lookups.contains("THE FILE SAID THIS"));
        assert_eq!(executed.lock().await.len(), MAX_RESOLVER_ROUNDS as usize);
    }

    /// Resolution must never fail a compaction: a provider error during a lookup
    /// round leaves the summary path exactly as it was before this feature.
    #[tokio::test]
    async fn provider_error_abandons_resolution_without_failing() {
        let base_url = failing_server().await;
        let executed = Arc::new(AsyncMutex::new(Vec::new()));
        let cancel = tokio_util::sync::CancellationToken::new();
        let (client, config) = route(&base_url);

        let outcome = run_resolver_rounds(
            &client,
            &config,
            "session-1",
            &turns(),
            &FakeResolver {
                executed: executed.clone(),
            },
            &cancel,
        )
        .await;

        assert_eq!(outcome.rounds, 0);
        assert_eq!(outcome.calls, 0);
        assert!(outcome.lookups.is_none());
        assert!(outcome.early_summary.is_none());
        assert!(executed.lock().await.is_empty());
    }
}
