//! `wait_for` — wait for a shell condition or a fixed delay, without parking the turn.
//!
//! Three phases:
//! 1. an inline attempt (or a bounded sleep for a delay-only `until`), which resolves the common
//!    "already true" case with no round trip;
//! 2. a spawned watcher when the condition is not met yet, so the agent keeps working;
//! 3. a `TaskCompleted` notification when the watcher finishes, which the existing bridge turns
//!    into a wake.
//!
//! The tool never shells out to `sleep` or `timeout`: the loop is tokio and a hung attempt is
//! killed by the terminal actor at the per-attempt budget.

mod watcher;

pub mod types;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

pub use types::{
    DEFAULT_ATTEMPT_TIMEOUT, DEFAULT_MAX_TIMEOUT, DEFAULT_RETRY_INITIAL,
    DEFAULT_RETRY_JITTER_PERMILLE, DEFAULT_RETRY_MAX, DEFAULT_RETRY_MULTIPLIER, DEFAULT_TIMEOUT,
    WAIT_DESCRIPTION_PREFIX, WAIT_DISPLAY_PREFIX, WAIT_FOR_TOOL_NAME, WaitForError, WaitForInput,
    WaitForOutput, WaitForParams, WaitOutcome, parse_until,
};
pub use watcher::{
    CANCELLED_SIGNAL, TIMEOUT_SIGNAL, WaitForRegistry, WaitSlot, WatcherOwnership, WatcherSpec,
    spawn_watcher, watcher_task_id,
};

use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::resources::{Cwd, NotificationHandle, Params, SessionFolder, Terminal};
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::util::duration::format_duration;

/// Characters of attempt output echoed back to the model.
const OUTPUT_EXCERPT_CHARS: usize = 4_096;

#[derive(Debug, Default)]
pub struct WaitForTool;

impl crate::types::tool_metadata::ToolMetadata for WaitForTool {
    fn kind(&self) -> ToolKind {
        ToolKind::WaitFor
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        r#"Wait for a shell condition or a fixed delay. Prefer this over `sleep` and `timeout` inside commands.

`until` takes a duration (`"5s"`), a command whose exit code is the condition (`"curl -sf localhost:3000"`), or both (`"10s && curl -sf localhost:3000"` — the duration is the initial delay). Exit code 0 satisfies; any other exit code retries.

Why prefer it: a bare `sleep 30` blocks the turn and, past ~15s, gets auto-backgrounded so you have to poll for it; a `sleep 5 && check` loop burns turns. Here the first attempt runs inline, and if it fails the tool returns immediately — a background watcher keeps polling with backoff until `timeout`, you keep working, and you are woken when the condition is met. A duration-only `until` is a clean `sleep` replacement.

The watcher shows in the tasks pane under Watchers and can be cancelled there. Its `task_id` stays readable${%- if tools.by_kind.background_task_action %} through `${{ tools.by_kind.background_task_action }}`${%- endif %}: the last attempt while it runs, the final state after it ends."#
    }

    fn emitted_notifications(&self) -> &'static [&'static str] {
        &["TaskCompleted", "BashExecutionBackgrounded"]
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for WaitForTool {
    type Args = WaitForInput;
    type Output = WaitForOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(WAIT_FOR_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            WAIT_FOR_TOOL_NAME,
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: false,
            tool_scope: Some(xai_tool_protocol::ToolScope::Write),
            ..Default::default()
        }
    }

    #[tracing::instrument(name = "tool.wait_for", skip_all)]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: WaitForInput,
    ) -> Result<WaitForOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;

        let resources = shared_resources(&ctx)?;
        let until = parse_until(&input.until)?;

        let (
            terminal,
            notification_handle,
            cwd,
            session_folder,
            owner_session_id,
            registry,
            params,
        ) = {
            let mut res = resources.lock().await;
            let terminal = res.require::<Terminal>()?.0.clone();
            let notification_handle = res
                .get::<NotificationHandle>()
                .map(|h| h.0.clone())
                .unwrap_or_default();
            let cwd = res
                .get::<Cwd>()
                .map(|c| c.0.clone())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let session_folder = res
                .get::<SessionFolder>()
                .map(|f| f.0.clone())
                .unwrap_or_else(|| cwd.clone());
            let owner_session_id = res
                .get::<crate::types::resources::OwnerSessionId>()
                .map(|o| o.0.clone());
            if res.get::<Arc<WaitForRegistry>>().is_none() {
                res.insert(Arc::new(WaitForRegistry::default()));
            }
            let registry = res
                .get::<Arc<WaitForRegistry>>()
                .cloned()
                .expect("registry inserted above");
            let params = res.get::<Params<WaitForParams>>().map(|p| p.0.clone());
            (
                terminal,
                notification_handle,
                cwd,
                session_folder,
                owner_session_id,
                registry,
                params,
            )
        };
        let params = params.unwrap_or_default();

        let timeout = params.resolve_timeout(input.timeout);
        let wake = input.wake.unwrap_or(params.wake_on_timeout);
        let retry_initial = input.retry.unwrap_or(params.retry_initial);
        input.validate(&until, timeout, retry_initial, wake)?;

        let started = Instant::now();
        let deadline = started + timeout;
        let attempt_timeout = params.attempt_timeout;
        let task_id = watcher_task_id(ctx.call_id.as_str());
        let output_file = session_folder
            .join("terminal")
            .join(format!("{task_id}.log"));

        let mut attempts = 0u32;
        let mut last_exit_code = None;
        // Assigned on the command path; the delay-only path returns before it is read.
        let last_output;

        // A delay-only `until` is a clean sleep: no command to poll.
        if until.is_delay_only() {
            let delay = until.delay.unwrap_or(Duration::ZERO);
            if !delay.is_zero() {
                tokio::time::sleep(delay.min(timeout)).await;
            }
            return Ok(finish(
                WaitOutcome::Satisfied,
                &input,
                &until,
                attempts,
                started.elapsed(),
                None,
                String::new(),
                None,
            ));
        }

        let command = until
            .command
            .clone()
            .expect("a non-delay-only until always carries a command");

        // Initial delay, when the caller asked for one.
        if let Some(delay) = until.delay
            && !delay.is_zero()
        {
            let remaining = deadline.saturating_duration_since(Instant::now());
            tokio::time::sleep(delay.min(remaining)).await;
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(finish(
                WaitOutcome::TimedOut,
                &input,
                &until,
                attempts,
                started.elapsed(),
                None,
                String::new(),
                None,
            ));
        }

        let outcome = watcher::run_attempt(
            &terminal,
            &cwd,
            &command,
            remaining.min(attempt_timeout),
            output_file.clone(),
            &notification_handle,
            ctx.call_id.as_str(),
            owner_session_id.clone(),
        )
        .await;

        attempts += 1;
        let mut watcher_output_file = output_file.clone();
        match outcome {
            Ok(attempt) => {
                let satisfied = attempt.satisfied();
                last_exit_code = attempt.exit_code;
                last_output = excerpt(&attempt.output);
                watcher_output_file = attempt.output_file;
                if satisfied {
                    return Ok(finish(
                        WaitOutcome::Satisfied,
                        &input,
                        &until,
                        attempts,
                        started.elapsed(),
                        last_exit_code,
                        last_output,
                        None,
                    ));
                }
            }
            Err(error) => {
                last_output = excerpt(&error.to_string());
            }
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(finish(
                WaitOutcome::TimedOut,
                &input,
                &until,
                attempts,
                started.elapsed(),
                last_exit_code,
                last_output,
                None,
            ));
        }
        if !wake {
            // Inline-only: the deadline is still open, so this is not a timeout.
            return Ok(finish(
                WaitOutcome::NotSatisfied,
                &input,
                &until,
                attempts,
                started.elapsed(),
                last_exit_code,
                last_output,
                None,
            ));
        }

        notification_handle.send_backgrounded(crate::notification::BashExecutionBackgrounded {
            base: crate::notification::BashNotificationBase {
                tool_call_id: ctx.call_id.as_str().to_owned(),
                command: command.clone(),
                output: Vec::new(),
                total_bytes: 0,
                truncated: false,
                cwd: cwd.clone(),
            },
            output_file: watcher_output_file,
            task_id: task_id.clone(),
            monitor_description: Some(format!("{WAIT_DESCRIPTION_PREFIX}{command}")),
            description: None,
        });

        // The owner/handle cell is shared with the registry entry, so a parent
        // session that adopts this watcher can retarget both in place.
        let ownership = Arc::new(std::sync::Mutex::new(watcher::WatcherOwnership {
            owner_session_id: owner_session_id.clone(),
            handle: notification_handle.clone(),
        }));
        spawn_watcher(
            Arc::downgrade(&terminal),
            registry,
            cwd.clone(),
            WatcherSpec {
                task_id: task_id.clone(),
                command,
                cwd: cwd.clone(),
                deadline,
                retry_initial,
                retry_max: params.retry_max,
                retry_multiplier: if input.retry.is_some() {
                    1
                } else {
                    params.retry_multiplier
                },
                retry_jitter_permille: params.retry_jitter_permille,
                attempt_timeout,
                output_file,
                tool_call_id: ctx.call_id.as_str().to_owned(),
                owner_session_id,
                started_at: SystemTime::now(),
                ownership,
            },
        );

        Ok(finish(
            WaitOutcome::Watching,
            &input,
            &until,
            attempts,
            started.elapsed(),
            last_exit_code,
            last_output,
            Some(task_id),
        ))
    }
}

#[allow(clippy::too_many_arguments)]
fn finish(
    outcome: WaitOutcome,
    input: &WaitForInput,
    until: &types::Until,
    attempts: u32,
    elapsed: Duration,
    last_exit_code: Option<i32>,
    last_output: String,
    task_id: Option<String>,
) -> WaitForOutput {
    let mut last_output = last_output;
    if outcome == WaitOutcome::TimedOut {
        let hint = "the wait timed out without the condition holding";
        last_output = if last_output.is_empty() {
            hint.to_owned()
        } else {
            format!("{hint}\n{last_output}")
        };
    } else if outcome == WaitOutcome::NotSatisfied {
        let hint = "the condition did not hold; no watcher was kept (`wake: false`)";
        last_output = if last_output.is_empty() {
            hint.to_owned()
        } else {
            format!("{hint}\n{last_output}")
        };
    }
    WaitForOutput {
        outcome,
        until: input.until.clone(),
        delay: until.delay.map(format_duration),
        command: until.command.clone(),
        attempts,
        elapsed: format_duration(elapsed),
        last_exit_code,
        last_output,
        task_id,
    }
}

fn excerpt(text: &str) -> String {
    let trimmed = text.trim_end();
    if trimmed.chars().count() <= OUTPUT_EXCERPT_CHARS {
        return trimmed.to_owned();
    }
    let head: String = trimmed.chars().take(OUTPUT_EXCERPT_CHARS).collect();
    format!("{head}\n… [truncated]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_id_is_the_canonical_name() {
        let tool = WaitForTool;
        assert_eq!(xai_tool_runtime::Tool::id(&tool).as_str(), "wait_for");
    }

    #[test]
    fn kind_and_namespace_are_wired() {
        use crate::types::tool_metadata::ToolMetadata;
        let tool = WaitForTool;
        assert_eq!(tool.kind(), ToolKind::WaitFor);
        assert_eq!(tool.tool_namespace(), ToolNamespace::GrokBuild);
        assert!(!tool.is_read_only());
    }

    #[test]
    fn description_mandates_preferring_it_over_sleep() {
        use crate::types::tool_metadata::ToolMetadata;
        let description = WaitForTool.description_template();
        assert!(
            description.contains("Prefer this over `sleep` and `timeout`"),
            "the preference must be explicit: {description}"
        );
        assert!(
            description.contains("sleep 5 && check"),
            "the anti-loop reason must be stated: {description}"
        );
        assert!(
            description.contains("auto-backgrounded"),
            "the why must name the bash auto-background trap: {description}"
        );
    }

    #[test]
    fn excerpt_passes_short_text_and_truncates_long() {
        assert_eq!(excerpt("hello"), "hello");
        let long = "x".repeat(OUTPUT_EXCERPT_CHARS + 10);
        assert!(excerpt(&long).ends_with("[truncated]"));
    }

    /// The description names the read-back tool, so the model knows a watcher id
    /// is not a dead end. It must survive a toolset without that tool.
    #[test]
    fn description_names_the_read_back_tool_only_when_present() {
        use crate::types::template_renderer::TemplateRenderer;
        use crate::types::tool_metadata::ToolMetadata;

        let template = WaitForTool.description_template();
        let with_tool = TemplateRenderer::new(
            std::collections::HashMap::from([(
                ToolKind::BackgroundTaskAction,
                "get_command_or_subagent_output".to_string(),
            )]),
            std::collections::HashMap::new(),
        );
        let rendered = with_tool.render(template).unwrap();
        assert!(
            rendered.contains("`get_command_or_subagent_output`"),
            "the read path must be named: {rendered}"
        );
        assert!(
            !rendered.contains("${%-"),
            "no raw template left: {rendered}"
        );

        let without = TemplateRenderer::new(
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        );
        let rendered = without.render(template).unwrap();
        assert!(
            rendered.contains("stays readable: the last attempt"),
            "the sentence must still read without the tool: {rendered}"
        );
        assert!(
            !rendered.contains("${%-"),
            "no raw template left: {rendered}"
        );
    }
}
