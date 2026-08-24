//! Strict profile-wide budget tracking.
//!
//! Budgets are aggregate across the ordered fallback chain **and** across
//! embed+rerank stages of one `retrieve` call. An arbitrary fallback list
//! cannot multiply latency above the profile deadline or attempt budget.
//! PR16 internal retries do not multiply the profile attempt counter.

use std::time::{Duration, Instant};

use super::error::{BudgetKind, LimitKind, OrchestratorError, OrchestratorResult, RetrievalStage};

/// Rough token estimate from UTF-8 bytes (4 bytes ≈ 1 token). Secret-free.
pub fn estimate_tokens_from_bytes(bytes: usize) -> u32 {
    let tokens = bytes.div_ceil(4);
    u32::try_from(tokens).unwrap_or(u32::MAX)
}

/// Sum of character lengths across texts.
pub fn total_input_bytes<'a, I>(texts: I) -> usize
where
    I: IntoIterator<Item = &'a str>,
{
    texts.into_iter().map(str::len).sum()
}

/// Profile budget limits frozen from a published snapshot profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileBudgetLimits {
    pub deadline: Duration,
    pub max_attempts: u32,
    pub max_input_tokens: u32,
    pub max_output_tokens: u32,
    pub max_input_bytes: usize,
    pub max_response_bytes: usize,
    pub max_candidates: u32,
    pub max_results: u32,
    pub max_batch_documents: u32,
    pub max_semantic_shortlist: u32,
    pub max_rerank_shortlist: u32,
}

impl ProfileBudgetLimits {
    /// Build limits from a PR15 profile config with hard caps.
    pub fn from_profile(
        profile: &xai_grok_config_types::RetrievalProfileConfig,
        max_batch_documents: u32,
    ) -> Self {
        let deadline = Duration::from_millis(profile.deadline_ms.max(1));
        // Response/output bytes: scale from max_output_tokens and clamp to
        // PR16 default response ceiling.
        let max_response_bytes = (profile.max_output_tokens as usize)
            .saturating_mul(16)
            .clamp(
                64 * 1024,
                xai_grok_inference::retrieval::DEFAULT_MAX_RESPONSE_BYTES,
            );
        let max_input_bytes = (profile.max_input_tokens as usize)
            .saturating_mul(8)
            .clamp(4 * 1024, 32 * 1024 * 1024);
        let shortlist = profile.max_candidates.min(profile.max_results.max(1) * 4);
        Self {
            deadline,
            max_attempts: profile.max_attempts.max(1),
            max_input_tokens: profile.max_input_tokens.max(1),
            max_output_tokens: profile.max_output_tokens.max(1),
            max_input_bytes,
            max_response_bytes,
            max_candidates: profile.max_candidates.max(1),
            max_results: profile.max_results.max(1),
            max_batch_documents: max_batch_documents.max(1),
            max_semantic_shortlist: shortlist.max(1),
            max_rerank_shortlist: profile.max_candidates.max(1),
        }
    }
}

/// Mutable running budget for one pipeline invocation (shared across stages).
#[derive(Debug, Clone)]
pub struct ProfileBudgetTracker {
    pub profile_id: String,
    pub limits: ProfileBudgetLimits,
    pub started_at: Instant,
    pub attempts_used: u32,
    pub input_bytes_used: usize,
    pub input_tokens_used: u32,
    pub output_bytes_used: usize,
    pub output_tokens_used: u32,
}

impl ProfileBudgetTracker {
    pub fn new(profile_id: impl Into<String>, limits: ProfileBudgetLimits, now: Instant) -> Self {
        Self {
            profile_id: profile_id.into(),
            limits,
            started_at: now,
            attempts_used: 0,
            input_bytes_used: 0,
            input_tokens_used: 0,
            output_bytes_used: 0,
            output_tokens_used: 0,
        }
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        let elapsed = now.saturating_duration_since(self.started_at);
        self.limits.deadline.saturating_sub(elapsed)
    }

    pub fn deadline_instant(&self) -> Instant {
        self.started_at + self.limits.deadline
    }

    pub fn ensure_not_expired(
        &self,
        now: Instant,
        stage: RetrievalStage,
    ) -> OrchestratorResult<()> {
        if self.remaining(now).is_zero() {
            return Err(OrchestratorError::DeadlineExceeded {
                profile_id: self.profile_id.clone(),
                stage,
            });
        }
        Ok(())
    }

    /// Effective per-route timeout = min(route_timeout, remaining deadline).
    pub fn effective_timeout(&self, now: Instant, route_timeout: Duration) -> Duration {
        let rem = self.remaining(now);
        if rem.is_zero() {
            Duration::ZERO
        } else if route_timeout.is_zero() {
            rem
        } else {
            rem.min(route_timeout)
        }
    }

    /// Reserve one profile-level route attempt (not PR16 internal retries).
    pub fn consume_attempt(&mut self, stage: RetrievalStage) -> OrchestratorResult<()> {
        if self.attempts_used >= self.limits.max_attempts {
            return Err(OrchestratorError::AttemptBudgetExceeded {
                profile_id: self.profile_id.clone(),
                stage,
                max_attempts: self.limits.max_attempts,
            });
        }
        self.attempts_used = self.attempts_used.saturating_add(1);
        Ok(())
    }

    pub fn attempts_remaining(&self) -> u32 {
        self.limits.max_attempts.saturating_sub(self.attempts_used)
    }

    pub fn charge_input(&mut self, bytes: usize, stage: RetrievalStage) -> OrchestratorResult<()> {
        let _ = stage;
        let next_bytes = self.input_bytes_used.saturating_add(bytes);
        if next_bytes > self.limits.max_input_bytes {
            return Err(OrchestratorError::InputBudgetExceeded {
                profile_id: self.profile_id.clone(),
                kind: BudgetKind::InputBytes,
            });
        }
        let tokens = estimate_tokens_from_bytes(bytes);
        let next_tokens = self.input_tokens_used.saturating_add(tokens);
        if next_tokens > self.limits.max_input_tokens {
            return Err(OrchestratorError::InputBudgetExceeded {
                profile_id: self.profile_id.clone(),
                kind: BudgetKind::InputTokens,
            });
        }
        self.input_bytes_used = next_bytes;
        self.input_tokens_used = next_tokens;
        Ok(())
    }

    /// Charge cumulative response/output size. Fail closed when either
    /// response-byte or output-token budget is exceeded.
    pub fn charge_output_bytes(&mut self, bytes: usize) -> OrchestratorResult<()> {
        let next_bytes = self.output_bytes_used.saturating_add(bytes);
        if next_bytes > self.limits.max_response_bytes {
            return Err(OrchestratorError::OutputBudgetExceeded {
                profile_id: self.profile_id.clone(),
                kind: BudgetKind::ResponseBytes,
            });
        }
        let tokens = estimate_tokens_from_bytes(bytes);
        let next_tokens = self.output_tokens_used.saturating_add(tokens);
        if next_tokens > self.limits.max_output_tokens {
            return Err(OrchestratorError::OutputBudgetExceeded {
                profile_id: self.profile_id.clone(),
                kind: BudgetKind::OutputTokens,
            });
        }
        self.output_bytes_used = next_bytes;
        self.output_tokens_used = next_tokens;
        Ok(())
    }

    pub fn check_candidate_limit(&self, count: usize) -> OrchestratorResult<()> {
        let actual = u32::try_from(count).unwrap_or(u32::MAX);
        if actual > self.limits.max_candidates {
            return Err(OrchestratorError::LimitExceeded {
                profile_id: self.profile_id.clone(),
                kind: LimitKind::Candidates,
                limit: self.limits.max_candidates,
                actual,
            });
        }
        Ok(())
    }

    pub fn check_result_limit(&self, count: usize) -> OrchestratorResult<()> {
        let actual = u32::try_from(count).unwrap_or(u32::MAX);
        if actual > self.limits.max_results {
            return Err(OrchestratorError::LimitExceeded {
                profile_id: self.profile_id.clone(),
                kind: LimitKind::Results,
                limit: self.limits.max_results,
                actual,
            });
        }
        Ok(())
    }

    pub fn check_batch_documents(&self, count: usize) -> OrchestratorResult<()> {
        let actual = u32::try_from(count).unwrap_or(u32::MAX);
        if actual > self.limits.max_batch_documents {
            return Err(OrchestratorError::LimitExceeded {
                profile_id: self.profile_id.clone(),
                kind: LimitKind::BatchDocuments,
                limit: self.limits.max_batch_documents,
                actual,
            });
        }
        Ok(())
    }

    pub fn clamp_results(&self, count: usize) -> usize {
        count.min(self.limits.max_results as usize)
    }

    pub fn clamp_candidates(&self, count: usize) -> usize {
        count.min(self.limits.max_candidates as usize)
    }

    pub fn clamp_rerank_shortlist(&self, count: usize) -> usize {
        count.min(self.limits.max_rerank_shortlist as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn limits() -> ProfileBudgetLimits {
        ProfileBudgetLimits {
            deadline: Duration::from_millis(1000),
            max_attempts: 2,
            max_input_tokens: 100,
            max_output_tokens: 50,
            max_input_bytes: 400,
            max_response_bytes: 1024,
            max_candidates: 10,
            max_results: 5,
            max_batch_documents: 8,
            max_semantic_shortlist: 10,
            max_rerank_shortlist: 10,
        }
    }

    #[test]
    fn attempt_budget_is_aggregate_not_multiplied() {
        let now = Instant::now();
        let mut t = ProfileBudgetTracker::new("p", limits(), now);
        t.consume_attempt(RetrievalStage::Embed).unwrap();
        t.consume_attempt(RetrievalStage::Embed).unwrap();
        let err = t.consume_attempt(RetrievalStage::Embed).unwrap_err();
        assert!(matches!(
            err,
            OrchestratorError::AttemptBudgetExceeded {
                max_attempts: 2,
                ..
            }
        ));
    }

    #[test]
    fn deadline_and_effective_timeout() {
        let start = Instant::now();
        let t = ProfileBudgetTracker::new("p", limits(), start);
        let half = start + Duration::from_millis(500);
        assert_eq!(
            t.effective_timeout(half, Duration::from_millis(800)),
            Duration::from_millis(500)
        );
        let past = start + Duration::from_millis(2000);
        assert!(t.remaining(past).is_zero());
        assert!(t.ensure_not_expired(past, RetrievalStage::Embed).is_err());
    }

    #[test]
    fn input_budget_charges() {
        let mut t = ProfileBudgetTracker::new("p", limits(), Instant::now());
        t.charge_input(100, RetrievalStage::Embed).unwrap();
        let err = t.charge_input(400, RetrievalStage::Embed).unwrap_err();
        assert!(matches!(
            err,
            OrchestratorError::InputBudgetExceeded {
                kind: BudgetKind::InputBytes,
                ..
            }
        ));
    }

    #[test]
    fn output_tokens_accumulate_and_fail_closed() {
        let mut t = ProfileBudgetTracker::new("p", limits(), Instant::now());
        // 50 tokens max; 200 bytes ≈ 50 tokens.
        t.charge_output_bytes(200).unwrap();
        let err = t.charge_output_bytes(8).unwrap_err();
        assert!(matches!(
            err,
            OrchestratorError::OutputBudgetExceeded {
                kind: BudgetKind::OutputTokens,
                ..
            }
        ));
        assert_eq!(t.output_tokens_used, 50);
    }

    #[test]
    fn output_response_bytes_fail_closed() {
        let mut lim = limits();
        lim.max_response_bytes = 100;
        lim.max_output_tokens = 10_000;
        let mut t = ProfileBudgetTracker::new("p", lim, Instant::now());
        let err = t.charge_output_bytes(101).unwrap_err();
        assert!(matches!(
            err,
            OrchestratorError::OutputBudgetExceeded {
                kind: BudgetKind::ResponseBytes,
                ..
            }
        ));
    }
}
