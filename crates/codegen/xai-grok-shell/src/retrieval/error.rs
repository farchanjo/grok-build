//! Typed retrieval orchestration errors and degradation outcomes.
//!
//! Display/Debug never include credentials, prompts, query/document text,
//! vectors, raw response bodies, env names/values, or custom account URLs.

use std::fmt;

use xai_grok_inference::RetrievalError;

/// Stage within a retrieval profile pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RetrievalStage {
    Embed,
    Rerank,
    Candidates,
    Orchestrate,
}

impl RetrievalStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Embed => "embed",
            Self::Rerank => "rerank",
            Self::Candidates => "candidates",
            Self::Orchestrate => "orchestrate",
        }
    }
}

/// Safe degradation kind when semantic routes are unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DegradationKind {
    /// All embedding routes failed/timeout/cooldown; lexical/native order used.
    SemanticUnavailable,
    /// All rerankers failed; pre-rerank ordering preserved exactly.
    RerankUnavailable,
    /// Profile missing or service disabled.
    ServiceDisabled,
    /// Profile referenced but not present in the published snapshot.
    ProfileMissing,
    /// Budget exhausted before a usable semantic result.
    BudgetExhausted,
}

impl DegradationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SemanticUnavailable => "semantic_unavailable",
            Self::RerankUnavailable => "rerank_unavailable",
            Self::ServiceDisabled => "service_disabled",
            Self::ProfileMissing => "profile_missing",
            Self::BudgetExhausted => "budget_exhausted",
        }
    }
}

/// Orchestrator-level error (secret-free).
#[derive(Clone, PartialEq, Eq)]
pub enum OrchestratorError {
    /// Retrieval service has no published graph (empty/disabled).
    ServiceDisabled,
    /// Named profile is not in the current snapshot.
    ProfileMissing { profile_id: String },
    /// Profile-wide absolute deadline exceeded.
    DeadlineExceeded {
        profile_id: String,
        stage: RetrievalStage,
    },
    /// Profile-wide attempt budget exhausted.
    AttemptBudgetExceeded {
        profile_id: String,
        stage: RetrievalStage,
        max_attempts: u32,
    },
    /// Aggregate input byte/token budget exceeded.
    InputBudgetExceeded {
        profile_id: String,
        kind: BudgetKind,
    },
    /// Aggregate output/response budget exceeded.
    OutputBudgetExceeded {
        profile_id: String,
        kind: BudgetKind,
    },
    /// Candidate or result limit violated by the caller.
    LimitExceeded {
        profile_id: String,
        kind: LimitKind,
        limit: u32,
        actual: u32,
    },
    /// Cancellation requested.
    Cancelled {
        profile_id: String,
        stage: RetrievalStage,
    },
    /// All configured routes failed for a stage (when hard error is requested).
    AllRoutesFailed {
        profile_id: String,
        stage: RetrievalStage,
        last_failure: Option<RouteFailureClass>,
    },
    /// Snapshot generation mismatch for a pinned call.
    GenerationMismatch { expected: u64, live: u64 },
    /// Invalid caller request (bounds, empty inputs, etc.).
    InvalidRequest(String),
    /// Underlying exact-route / adapter failure (already secret-free).
    Route(RetrievalError),
    /// Internal configuration/build issue (never panics the process).
    Config(String),
}

/// Which aggregate budget dimension was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BudgetKind {
    InputBytes,
    InputTokens,
    OutputBytes,
    OutputTokens,
    ResponseBytes,
}

impl BudgetKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InputBytes => "input_bytes",
            Self::InputTokens => "input_tokens",
            Self::OutputBytes => "output_bytes",
            Self::OutputTokens => "output_tokens",
            Self::ResponseBytes => "response_bytes",
        }
    }
}

/// Candidate/result limit kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LimitKind {
    Candidates,
    Results,
    BatchDocuments,
    SemanticShortlist,
    RerankShortlist,
}

impl LimitKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Candidates => "candidates",
            Self::Results => "results",
            Self::BatchDocuments => "batch_documents",
            Self::SemanticShortlist => "semantic_shortlist",
            Self::RerankShortlist => "rerank_shortlist",
        }
    }
}

/// Coarse route failure class for telemetry (no bodies/credentials).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteFailureClass {
    Auth,
    Config,
    Capability,
    RouteGuard,
    Timeout,
    RateLimited,
    Transport,
    Malformed,
    Cancelled,
    Deadline,
    Other,
}

impl RouteFailureClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::Config => "config",
            Self::Capability => "capability",
            Self::RouteGuard => "route_guard",
            Self::Timeout => "timeout",
            Self::RateLimited => "rate_limited",
            Self::Transport => "transport",
            Self::Malformed => "malformed",
            Self::Cancelled => "cancelled",
            Self::Deadline => "deadline",
            Self::Other => "other",
        }
    }

    /// Classify a PR16 [`RetrievalError`] without retaining message text.
    pub fn from_retrieval_error(err: &RetrievalError) -> Self {
        match err {
            RetrievalError::MissingCredential
            | RetrievalError::Http {
                category:
                    xai_grok_inference::RetrievalErrorCategory::Authentication
                    | xai_grok_inference::RetrievalErrorCategory::Authorization,
                ..
            } => Self::Auth,
            RetrievalError::InvalidRequest(_)
            | RetrievalError::InvalidUrl(_)
            | RetrievalError::ProtocolMismatch(_)
            | RetrievalError::SurfaceMismatch(_) => Self::Config,
            RetrievalError::CapabilityDenied(_) => Self::Capability,
            RetrievalError::RedirectPolicy(_) => Self::RouteGuard,
            RetrievalError::Timeout => Self::Timeout,
            RetrievalError::RateLimited { .. }
            | RetrievalError::Http {
                category: xai_grok_inference::RetrievalErrorCategory::RateLimit,
                ..
            } => Self::RateLimited,
            RetrievalError::Transport(_)
            | RetrievalError::Http {
                category: xai_grok_inference::RetrievalErrorCategory::Server,
                ..
            } => Self::Transport,
            RetrievalError::Decode(_) | RetrievalError::MalformedResponse(_) => Self::Malformed,
            RetrievalError::Cancelled => Self::Cancelled,
            RetrievalError::DeadlineExceeded => Self::Deadline,
            RetrievalError::OversizedResponse { .. } | RetrievalError::Http { .. } => Self::Other,
        }
    }

    /// Whether the failure may allow trying the next explicitly configured route.
    pub fn allows_explicit_fallback(self) -> bool {
        !matches!(self, Self::Cancelled)
    }

    /// Whether the failure should enter per-route cooldown (retryable class only).
    pub fn is_cooldown_eligible(self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::RateLimited | Self::Transport | Self::Deadline
        )
    }

    /// Permanent/auth/config: skip for this call but do not create broad cooldowns.
    pub fn is_terminal_for_route(self) -> bool {
        matches!(
            self,
            Self::Auth | Self::Config | Self::Capability | Self::RouteGuard | Self::Malformed
        )
    }
}

impl fmt::Debug for OrchestratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ServiceDisabled => f.write_str("ServiceDisabled"),
            Self::ProfileMissing { profile_id } => f
                .debug_struct("ProfileMissing")
                .field("profile_id", profile_id)
                .finish(),
            Self::DeadlineExceeded { profile_id, stage } => f
                .debug_struct("DeadlineExceeded")
                .field("profile_id", profile_id)
                .field("stage", stage)
                .finish(),
            Self::AttemptBudgetExceeded {
                profile_id,
                stage,
                max_attempts,
            } => f
                .debug_struct("AttemptBudgetExceeded")
                .field("profile_id", profile_id)
                .field("stage", stage)
                .field("max_attempts", max_attempts)
                .finish(),
            Self::InputBudgetExceeded { profile_id, kind } => f
                .debug_struct("InputBudgetExceeded")
                .field("profile_id", profile_id)
                .field("kind", kind)
                .finish(),
            Self::OutputBudgetExceeded { profile_id, kind } => f
                .debug_struct("OutputBudgetExceeded")
                .field("profile_id", profile_id)
                .field("kind", kind)
                .finish(),
            Self::LimitExceeded {
                profile_id,
                kind,
                limit,
                actual,
            } => f
                .debug_struct("LimitExceeded")
                .field("profile_id", profile_id)
                .field("kind", kind)
                .field("limit", limit)
                .field("actual", actual)
                .finish(),
            Self::Cancelled { profile_id, stage } => f
                .debug_struct("Cancelled")
                .field("profile_id", profile_id)
                .field("stage", stage)
                .finish(),
            Self::AllRoutesFailed {
                profile_id,
                stage,
                last_failure,
            } => f
                .debug_struct("AllRoutesFailed")
                .field("profile_id", profile_id)
                .field("stage", stage)
                .field("last_failure", last_failure)
                .finish(),
            Self::GenerationMismatch { expected, live } => f
                .debug_struct("GenerationMismatch")
                .field("expected", expected)
                .field("live", live)
                .finish(),
            Self::InvalidRequest(m) => f.debug_tuple("InvalidRequest").field(m).finish(),
            Self::Route(e) => f.debug_tuple("Route").field(e).finish(),
            Self::Config(m) => f.debug_tuple("Config").field(m).finish(),
        }
    }
}

impl fmt::Display for OrchestratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ServiceDisabled => write!(
                f,
                "retrieval service is disabled (no validated graph published)"
            ),
            Self::ProfileMissing { profile_id } => {
                write!(f, "retrieval profile `{profile_id}` is not configured")
            }
            Self::DeadlineExceeded { profile_id, stage } => write!(
                f,
                "retrieval profile `{profile_id}` deadline exceeded during {}",
                stage.as_str()
            ),
            Self::AttemptBudgetExceeded {
                profile_id,
                stage,
                max_attempts,
            } => write!(
                f,
                "retrieval profile `{profile_id}` attempt budget ({max_attempts}) exceeded during {}",
                stage.as_str()
            ),
            Self::InputBudgetExceeded { profile_id, kind } => write!(
                f,
                "retrieval profile `{profile_id}` input budget exceeded ({})",
                kind.as_str()
            ),
            Self::OutputBudgetExceeded { profile_id, kind } => write!(
                f,
                "retrieval profile `{profile_id}` output budget exceeded ({})",
                kind.as_str()
            ),
            Self::LimitExceeded {
                profile_id,
                kind,
                limit,
                actual,
            } => write!(
                f,
                "retrieval profile `{profile_id}` {} limit {limit} exceeded (actual {actual})",
                kind.as_str()
            ),
            Self::Cancelled { profile_id, stage } => write!(
                f,
                "retrieval profile `{profile_id}` cancelled during {}",
                stage.as_str()
            ),
            Self::AllRoutesFailed {
                profile_id, stage, ..
            } => write!(
                f,
                "all configured {} routes failed for profile `{profile_id}`",
                stage.as_str()
            ),
            Self::GenerationMismatch { expected, live } => write!(
                f,
                "retrieval snapshot generation mismatch (expected {expected}, live {live})"
            ),
            Self::InvalidRequest(m) => write!(f, "invalid retrieval request: {m}"),
            Self::Route(e) => write!(f, "{e}"),
            Self::Config(m) => write!(f, "retrieval config: {m}"),
        }
    }
}

impl std::error::Error for OrchestratorError {}

impl From<RetrievalError> for OrchestratorError {
    fn from(value: RetrievalError) -> Self {
        match value {
            RetrievalError::Cancelled => Self::Cancelled {
                profile_id: String::new(),
                stage: RetrievalStage::Orchestrate,
            },
            RetrievalError::DeadlineExceeded => Self::DeadlineExceeded {
                profile_id: String::new(),
                stage: RetrievalStage::Orchestrate,
            },
            other => Self::Route(other),
        }
    }
}

/// Soft degradation attached to a successful (or partial) pipeline result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradationNotice {
    pub kind: DegradationKind,
    pub profile_id: String,
    pub stage: RetrievalStage,
    pub last_failure: Option<RouteFailureClass>,
}

impl DegradationNotice {
    pub fn new(
        kind: DegradationKind,
        profile_id: impl Into<String>,
        stage: RetrievalStage,
        last_failure: Option<RouteFailureClass>,
    ) -> Self {
        Self {
            kind,
            profile_id: profile_id.into(),
            stage,
            last_failure,
        }
    }
}

/// Result type for orchestrator APIs.
pub type OrchestratorResult<T> = Result<T, OrchestratorError>;
