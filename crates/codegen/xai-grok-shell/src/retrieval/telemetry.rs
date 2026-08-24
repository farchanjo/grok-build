//! Sanitized retrieval orchestration telemetry.
//!
//! Emits only profile/provider IDs, purpose/stage, route index, generations,
//! counts, durations, attempt/budget flags, and safe failure/degradation enums.
//! Never credentials, env names/values, prompt/query/doc text, skill bodies,
//! vectors, raw response bodies, org/project/custom URLs, or account PII.

use std::fmt;
use std::time::Duration;

use super::error::{DegradationKind, RetrievalStage, RouteFailureClass};

/// One sanitized pipeline event (safe for logs / Debug).
#[derive(Clone, PartialEq, Eq)]
pub struct RetrievalTelemetryEvent {
    pub profile_id: String,
    pub stage: RetrievalStage,
    pub purpose: &'static str,
    pub route_index: Option<u32>,
    pub route_model_id: Option<String>,
    pub provider_instance_id: Option<String>,
    pub snapshot_generation: u64,
    pub provider_generation: Option<u64>,
    pub attempt: u32,
    pub max_attempts: u32,
    pub duration_ms: Option<u64>,
    pub input_count: Option<u32>,
    pub result_count: Option<u32>,
    pub budget_flags: BudgetFlags,
    pub outcome: TelemetryOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BudgetFlags {
    pub deadline_hit: bool,
    pub attempt_budget_hit: bool,
    pub input_budget_hit: bool,
    pub output_budget_hit: bool,
    pub cancelled: bool,
    pub cooldown_skip: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryOutcome {
    Success,
    RouteFailure(RouteFailureClass),
    Degraded(DegradationKind),
    HardError,
    SkippedCooldown,
    SkippedBudget,
}

impl fmt::Debug for RetrievalTelemetryEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetrievalTelemetryEvent")
            .field("profile_id", &self.profile_id)
            .field("stage", &self.stage)
            .field("purpose", &self.purpose)
            .field("route_index", &self.route_index)
            .field("route_model_id", &self.route_model_id)
            .field("provider_instance_id", &self.provider_instance_id)
            .field("snapshot_generation", &self.snapshot_generation)
            .field("provider_generation", &self.provider_generation)
            .field("attempt", &self.attempt)
            .field("max_attempts", &self.max_attempts)
            .field("duration_ms", &self.duration_ms)
            .field("input_count", &self.input_count)
            .field("result_count", &self.result_count)
            .field("budget_flags", &self.budget_flags)
            .field("outcome", &self.outcome)
            .finish()
    }
}

/// Sink for telemetry (tests capture; production logs at debug).
pub trait TelemetrySink: Send + Sync + 'static {
    fn emit(&self, event: RetrievalTelemetryEvent);
}

/// Default sink: tracing debug only (no sensitive fields exist on the event).
#[derive(Debug, Default)]
pub struct TracingTelemetrySink;

impl TelemetrySink for TracingTelemetrySink {
    fn emit(&self, event: RetrievalTelemetryEvent) {
        tracing::debug!(
            target: "retrieval_orchestrator",
            profile_id = %event.profile_id,
            stage = event.stage.as_str(),
            purpose = event.purpose,
            route_index = ?event.route_index,
            route_model_id = ?event.route_model_id,
            provider_instance_id = ?event.provider_instance_id,
            snapshot_generation = event.snapshot_generation,
            attempt = event.attempt,
            max_attempts = event.max_attempts,
            duration_ms = ?event.duration_ms,
            outcome = ?event.outcome,
            "retrieval orchestrator event"
        );
    }
}

/// Recording sink for tests.
#[derive(Default)]
pub struct RecordingTelemetrySink {
    events: parking_lot::Mutex<Vec<RetrievalTelemetryEvent>>,
}

impl RecordingTelemetrySink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<RetrievalTelemetryEvent> {
        self.events.lock().clone()
    }

    pub fn clear(&self) {
        self.events.lock().clear();
    }
}

impl TelemetrySink for RecordingTelemetrySink {
    fn emit(&self, event: RetrievalTelemetryEvent) {
        self.events.lock().push(event);
    }
}

pub fn duration_ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Redaction canary: event Debug must not contain common secret patterns.
#[cfg(test)]
pub fn debug_is_redacted(event: &RetrievalTelemetryEvent) -> bool {
    let s = format!("{event:?}");
    !s.contains("sk-")
        && !s.contains("Bearer ")
        && !s.contains("api_key")
        && !s.contains("OPENAI_API_KEY")
        && !s.contains("Authorization")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_debug_has_no_bodies() {
        let ev = RetrievalTelemetryEvent {
            profile_id: "default".into(),
            stage: RetrievalStage::Embed,
            purpose: "embeddings",
            route_index: Some(0),
            route_model_id: Some("emb-a".into()),
            provider_instance_id: Some("acct-a".into()),
            snapshot_generation: 3,
            provider_generation: Some(7),
            attempt: 1,
            max_attempts: 2,
            duration_ms: Some(12),
            input_count: Some(2),
            result_count: Some(2),
            budget_flags: BudgetFlags::default(),
            outcome: TelemetryOutcome::Success,
        };
        assert!(debug_is_redacted(&ev));
    }
}
