//! `asset_job_subscribe` — follow a transfer job's progress as an event stream.
//!
//! Read-only. The stream is the shell's own primitive (`AssetJobRegistry::subscribe`):
//! progress is coalesced to one event per `buffer_bytes` moved, throttled to one
//! event per `interval_ms`, and rate-limited by a token bucket, so a fast local
//! copy that produces thousands of chunks yields a handful of events.
//!
//! The tool drains that stream for up to `timeout_secs` and returns the events.
//! With `until_complete` the stream stays open past `max_events` until the job
//! ends; without it the stream closes at `max_events` and the last event is
//! marked `cutoff`.

use std::time::Duration;

use xai_file_utils::assets::{JobEvent, JobState, SubscribeOptions};

use super::{parse_job_id, require_jobs, unknown_job};
use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Canonical tool name.
pub const ASSET_JOB_SUBSCRIBE_TOOL_NAME: &str = "asset_job_subscribe";

/// Default drain window when `timeout_secs` is absent.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Longest drain window.
pub const MAX_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AssetJobSubscribeInput {
    #[schemars(description = "Job id returned by `asset_upload` or `asset_download`.")]
    pub job_id: String,

    #[serde(default)]
    #[schemars(
        description = "Minimum milliseconds between events. Default: 500. Range: 1..=60000."
    )]
    pub interval_ms: Option<u64>,

    #[serde(default)]
    #[schemars(
        description = "Coalescing window in bytes: an event is only emitted once this much more data has moved. Default: 65536."
    )]
    pub buffer_bytes: Option<usize>,

    #[serde(default)]
    #[schemars(description = "Rate-limit burst capacity in events. Default: 10. Range: 1..=1000.")]
    pub capacity: Option<u32>,

    #[serde(default)]
    #[schemars(
        description = "Stop after this many progress events. Default: 50. Range: 1..=1000."
    )]
    pub max_events: Option<usize>,

    #[serde(default)]
    #[schemars(
        description = "Keep the stream open past `max_events` until the job ends. Default: false."
    )]
    pub until_complete: bool,

    #[serde(default)]
    #[schemars(
        description = "Give up and return the events collected so far after this many seconds (0..=600). Default: 30."
    )]
    pub timeout_secs: Option<u64>,
}

/// Structured result of one subscription drain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetJobSubscribeOutput {
    pub job_id: String,
    /// Coalesced events, oldest first.
    pub events: Vec<JobEvent>,
    pub event_count: usize,
    /// Progress updates folded into the returned events.
    pub coalesced_events: u64,
    /// State reported by the last event.
    pub state: String,
    /// True when the last event carried a terminal state.
    pub terminal: bool,
    /// True when the stream closed on `max_events` before the job ended.
    pub truncated: bool,
    /// True when the drain window elapsed with the job still running.
    pub timed_out: bool,
    /// Pre-formatted model-facing prose.
    pub text: String,
}

#[derive(Debug, Default)]
pub struct AssetJobSubscribeTool;

impl crate::types::tool_metadata::ToolMetadata for AssetJobSubscribeTool {
    fn kind(&self) -> ToolKind {
        ToolKind::AssetJobSubscribe
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Follow an asset transfer job and return its coalesced progress events. Read-only. \
         Use `until_complete` to follow a job to the end, or `max_events` to sample it."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for AssetJobSubscribeTool {
    type Args = AssetJobSubscribeInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(ASSET_JOB_SUBSCRIBE_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            ASSET_JOB_SUBSCRIBE_TOOL_NAME,
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: true,
            tool_scope: Some(xai_tool_protocol::ToolScope::Read),
            ..Default::default()
        }
    }

    #[tracing::instrument(name = "tool.asset_job_subscribe", skip_all, fields(job = %input.job_id))]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: AssetJobSubscribeInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;

        let resources = shared_resources(&ctx)?;
        let registry = require_jobs(&resources).await?;
        let job_id = parse_job_id(&input.job_id)?;
        if registry.get(&job_id).await.is_none() {
            return Err(unknown_job(&job_id));
        }

        let options = resolve_options(&input)?;
        let timeout = match input.timeout_secs {
            None => Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            Some(secs) if secs <= MAX_TIMEOUT_SECS => Duration::from_secs(secs),
            Some(secs) => {
                return Err(xai_tool_runtime::ToolError::invalid_arguments(format!(
                    "`timeout_secs` must be 0..={MAX_TIMEOUT_SECS}, got {secs}"
                )));
            }
        };

        let mut subscription = registry.subscribe(&job_id, options);
        let mut events: Vec<JobEvent> = Vec::new();
        let mut timed_out = false;

        loop {
            match tokio::time::timeout(timeout, subscription.next()).await {
                Ok(Some(event)) => events.push(event),
                // Stream closed: the job ended or the cutoff was reached.
                Ok(None) => break,
                Err(_) => {
                    timed_out = true;
                    break;
                }
            }
        }

        let coalesced_events: u64 = events.iter().map(|event| event.coalesced_events).sum();
        let truncated = events.last().is_some_and(|event| event.cutoff);
        let terminal = events.last().is_some_and(|event| event.terminal);
        // With no events the job's own state is the honest fallback.
        let state = match events.last() {
            Some(event) => event.state.clone(),
            None => registry
                .get(&job_id)
                .await
                .map(|snapshot| snapshot.state.as_str().to_owned())
                .unwrap_or_else(|| JobState::Queued.as_str().to_owned()),
        };

        let text = if events.is_empty() {
            format!("Job `{job_id}` is `{state}`; no progress event was emitted in the window.")
        } else {
            let mut text = format!(
                "Job `{job_id}`: {} event{} ({} update{} coalesced), now `{state}`.",
                events.len(),
                if events.len() == 1 { "" } else { "s" },
                coalesced_events,
                if coalesced_events == 1 { "" } else { "s" },
            );
            for event in events.iter().rev().take(3).rev() {
                text.push_str("\n- ");
                text.push_str(&event.text);
            }
            if truncated {
                text.push_str("\nThe stream stopped at `max_events`; call again to continue.");
            } else if timed_out && !terminal {
                text.push_str("\nThe drain window elapsed with the job still running.");
            }
            text
        };

        Ok(ToolOutput::AssetJobSubscribe(AssetJobSubscribeOutput {
            job_id,
            event_count: events.len(),
            events,
            coalesced_events,
            state,
            terminal,
            truncated,
            timed_out,
            text,
        }))
    }
}

/// Merge the caller's knobs over the defaults, then reject out-of-range ones.
fn resolve_options(
    input: &AssetJobSubscribeInput,
) -> Result<SubscribeOptions, xai_tool_runtime::ToolError> {
    let options = SubscribeOptions {
        interval_ms: input
            .interval_ms
            .unwrap_or(SubscribeOptions::default().interval_ms),
        buffer_bytes: input
            .buffer_bytes
            .unwrap_or(SubscribeOptions::default().buffer_bytes),
        capacity: input
            .capacity
            .unwrap_or(SubscribeOptions::default().capacity),
        max_events: input
            .max_events
            .unwrap_or(SubscribeOptions::default().max_events),
        until_complete: input.until_complete,
    };
    options.validate().map_err(|err| {
        xai_tool_runtime::ToolError::invalid_arguments(err.to_string())
            .with_details(serde_json::json!({ "code": err.code() }))
    })?;
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::grok_build::assets::test_support::resources_with_store;
    use crate::types::tool_metadata::test_ctx_with_call_id;
    use std::sync::Arc;
    use xai_file_utils::assets::{
        AssetJobRegistry, AssetKey, ContentType, MockAssetStore, PutRequest, SharedAssetStore,
    };

    fn input(job_id: &str) -> AssetJobSubscribeInput {
        AssetJobSubscribeInput {
            job_id: job_id.to_owned(),
            interval_ms: None,
            buffer_bytes: None,
            capacity: None,
            max_events: None,
            until_complete: false,
            timeout_secs: None,
        }
    }

    fn request(name: &str, size: usize) -> PutRequest {
        PutRequest::from_bytes(
            AssetKey::parse(&format!("uploads/{name}")).unwrap(),
            vec![7u8; size],
            ContentType::default(),
        )
    }

    async fn run(
        store: SharedAssetStore,
        cwd: &std::path::Path,
        input: AssetJobSubscribeInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        xai_tool_runtime::Tool::run(
            &AssetJobSubscribeTool,
            test_ctx_with_call_id(resources_with_store(store, cwd), "test-call"),
            input,
        )
        .await
    }

    #[test]
    fn tool_name_is_read_only() {
        let tool = AssetJobSubscribeTool;
        assert_eq!(
            xai_tool_runtime::Tool::id(&tool).as_str(),
            ASSET_JOB_SUBSCRIBE_TOOL_NAME
        );
        assert!(crate::types::tool_metadata::ToolMetadata::is_read_only(
            &tool
        ));
    }

    #[test]
    fn options_default_and_reject_out_of_range() {
        let options = resolve_options(&input("job")).unwrap();
        assert_eq!(options, SubscribeOptions::default());

        let mut bad = input("job");
        bad.interval_ms = Some(0);
        let err = resolve_options(&bad).unwrap_err();
        assert_eq!(err.details.unwrap()["code"], "asset_job_invalid_option");

        let mut bad = input("job");
        bad.max_events = Some(0);
        assert!(resolve_options(&bad).is_err());
    }

    #[tokio::test]
    async fn unknown_job_is_a_not_found_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = Arc::new(MockAssetStore::new());
        let err = run(mock, dir.path(), input("nope")).await.unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::NotFound);
    }

    #[tokio::test]
    async fn bad_timeout_is_an_argument_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = Arc::new(MockAssetStore::new());
        let mut bad = input("nope");
        bad.timeout_secs = Some(MAX_TIMEOUT_SECS + 1);
        // The job check runs first, so seed a real job.
        let store: SharedAssetStore = mock;
        let resources = resources_with_store(store.clone(), dir.path());
        let registry = Arc::new(AssetJobRegistry::new());
        {
            let mut res = resources.lock().await;
            res.insert(Arc::clone(&registry));
        }
        let job_id = registry.spawn_upload(store, request("a.bin", 8));
        registry
            .wait_for_completion(&job_id, Some(Duration::from_secs(5)))
            .await
            .unwrap();

        bad.job_id = job_id;
        let err = xai_tool_runtime::Tool::run(
            &AssetJobSubscribeTool,
            test_ctx_with_call_id(resources, "test-call"),
            bad,
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::InvalidArguments);
    }

    #[tokio::test]
    async fn follows_a_job_to_completion_and_coalesces_progress() {
        let dir = tempfile::TempDir::new().unwrap();
        // 16 progress steps, but a 256 KiB coalescing window: the subscriber
        // must see far fewer events than the adapter produced updates.
        let store: SharedAssetStore = Arc::new(
            MockAssetStore::builder()
                .progress_steps(16, Duration::from_millis(5))
                .build(),
        );
        let resources = resources_with_store(store.clone(), dir.path());
        let registry = Arc::new(AssetJobRegistry::new());
        {
            let mut res = resources.lock().await;
            res.insert(Arc::clone(&registry));
        }
        let job_id = registry.spawn_upload(store, request("big.bin", 512 * 1024));

        let mut subscribe_input = input(&job_id);
        subscribe_input.until_complete = true;
        subscribe_input.timeout_secs = Some(20);
        subscribe_input.buffer_bytes = Some(256 * 1024);
        subscribe_input.interval_ms = Some(1);

        let out = xai_tool_runtime::Tool::run(
            &AssetJobSubscribeTool,
            test_ctx_with_call_id(resources, "test-call"),
            subscribe_input,
        )
        .await
        .unwrap();

        match out {
            ToolOutput::AssetJobSubscribe(out) => {
                assert!(out.terminal, "followed the job to its terminal event");
                assert_eq!(out.state, "completed");
                assert!(!out.truncated);
                assert!(!out.timed_out);
                assert!(
                    out.event_count < 16,
                    "coalescing must shrink {} events below 16 chunks",
                    out.event_count
                );
                assert!(out.coalesced_events > 0, "coalescing must be reported");
                assert!(out.events.last().unwrap().terminal);
                assert!(out.text.contains("now `completed`"));
            }
            other => panic!("expected AssetJobSubscribe, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn max_events_cutoff_is_reported_honestly() {
        let dir = tempfile::TempDir::new().unwrap();
        // Paced progress keeps the job running while the subscriber drains, so
        // the cutoff path is deterministic rather than racing a fast mock.
        let store: SharedAssetStore = Arc::new(
            MockAssetStore::builder()
                .progress_steps(8, Duration::from_millis(50))
                .build(),
        );
        let resources = resources_with_store(store.clone(), dir.path());
        let registry = Arc::new(AssetJobRegistry::new());
        {
            let mut res = resources.lock().await;
            res.insert(Arc::clone(&registry));
        }
        let job_id = registry.spawn_upload(store, request("big.bin", 256 * 1024));

        let mut subscribe_input = input(&job_id);
        subscribe_input.until_complete = false;
        subscribe_input.max_events = Some(1);
        subscribe_input.buffer_bytes = Some(1);
        subscribe_input.capacity = Some(1000);
        subscribe_input.timeout_secs = Some(10);

        let out = xai_tool_runtime::Tool::run(
            &AssetJobSubscribeTool,
            test_ctx_with_call_id(resources, "test-call"),
            subscribe_input,
        )
        .await
        .unwrap();

        match out {
            ToolOutput::AssetJobSubscribe(out) => {
                assert!(out.truncated, "cutoff must be reported");
                assert!(out.event_count <= 2, "events: {}", out.event_count);
                assert!(out.text.contains("max_events"));
            }
            other => panic!("expected AssetJobSubscribe, got {other:?}"),
        }
    }
}
