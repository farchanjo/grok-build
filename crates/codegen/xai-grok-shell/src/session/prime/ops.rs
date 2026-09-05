//! Versioned, bounded, secret-free Prime index operations.
//!
//! ACP methods:
//! - `x.ai/prime/index/status` — generation/fingerprint-preconditioned refresh
//! - `x.ai/prime/index/backfill` — missing-only vectors
//! - `x.ai/prime/index/rebuild` — full restage
//! - `x.ai/prime/index/cancel`
//!
//! Configured-profile embedding is contacted only after
//! `confirmConfiguredProfile = true`. Payloads never carry prompts, bodies,
//! descriptions, vectors, full fingerprints, raw errors, paths, or credentials.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use xai_grok_memory::CollectionKind;
use xai_grok_memory::embedding::EmbeddingProvider;
use xai_grok_tools::implementations::skills::strict::SKILLS_API_VERSION;

use super::index::{
    FrozenEmbeddingPin, PinnedServiceEmbedder, PrimeIndexError, PrimeIndexHandle, prime_index_for,
};
use super::notify;
use crate::retrieval::registry_for_home;

/// ACP/JSON contract version for Prime index operations.
pub const PRIME_INDEX_API_VERSION: u32 = 1;
/// Notification schema version (independent of the method apiVersion).
pub const PRIME_INDEX_SCHEMA_VERSION: u32 = 1;

const FINGERPRINT_SHORT_LEN: usize = 12;
const MAX_FAILURE_LEN: usize = 80;
/// Profile ids only — short enough to fit `confirm_required:<id>` under
/// [`MAX_FAILURE_LEN`].
const MAX_ROUTE_DISPLAY_LEN: usize = 48;
/// Stable, mixed-version-safe reason code. Old pagers may still parse an
/// optional `:id` suffix; unsanitary routes never appear after the colon.
pub const PRIME_FAILURE_CONFIRM_REQUIRED: &str = "confirm_required";

/// Capability bits advertised on initialize and echoed on status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimeIndexCapabilities {
    pub status: bool,
    pub backfill: bool,
    pub rebuild: bool,
    pub cancel: bool,
}

impl PrimeIndexCapabilities {
    pub const SUPPORTED: Self = Self {
        status: true,
        backfill: true,
        rebuild: true,
        cancel: true,
    };

    pub const UNSUPPORTED: Self = Self {
        status: false,
        backfill: false,
        rebuild: false,
        cancel: false,
    };
}

impl Default for PrimeIndexCapabilities {
    fn default() -> Self {
        Self::UNSUPPORTED
    }
}

/// Compact per-collection index snapshot. Fingerprints are truncated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimeIndexCollectionStatus {
    /// `"skills"` or `"agents"`.
    pub collection: String,
    pub generation: u64,
    pub fingerprint_short: String,
    pub item_count: u64,
    pub vector_count: u64,
    pub missing_vectors: u64,
    /// `"ready"` | `"pending"` | `"unavailable"` | `"read_only"` | `"stale"`.
    pub readiness: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
}

/// Versioned Prime index status. Secret-free.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimeIndexStatus {
    pub api_version: u32,
    pub generation: u64,
    pub fingerprint_short: String,
    pub skills: PrimeIndexCollectionStatus,
    pub agents: PrimeIndexCollectionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<PrimeIndexJobStatus>,
    /// Configured retrieval profile id (never an endpoint or credential).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_route: Option<String>,
    pub capabilities: PrimeIndexCapabilities,
    /// True when the caller's generation+fingerprint already matched.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unchanged: bool,
}

/// In-flight or last-completed index job. Progress fits a compact footer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimeIndexJobStatus {
    pub api_version: u32,
    pub job_id: String,
    /// `"backfill"` | `"rebuild"`.
    pub kind: String,
    /// `"skills"` | `"agents"` | `"all"`.
    pub collection: String,
    /// `"running"` | `"cancelling"` | `"completed"` | `"failed"` | `"cancelled"`.
    pub state: String,
    pub generation: u64,
    pub fingerprint_short: String,
    pub done: u64,
    pub total: u64,
    /// Additive v1 field. Older job objects omit it; fail closed (do not
    /// treat a missing flag as confirmed).
    #[serde(default)]
    pub confirm_configured_profile: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configured_route: Option<String>,
    /// Bounded secret-free failure code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
}

impl PrimeIndexJobStatus {
    pub fn is_terminal(&self) -> bool {
        prime_job_state_is_terminal(&self.state)
    }

    /// Drop endpoints, credentials, paths, and control text from display
    /// fields. Idempotent; safe for mixed-version ingest.
    pub fn sanitize_secrets(&mut self) {
        self.configured_route =
            displayable_configured_route(self.configured_route.as_deref()).map(str::to_owned);
        self.failure = self.failure.as_deref().map(sanitize_prime_job_failure);
    }
}

impl PrimeIndexStatus {
    /// Drop endpoints, credentials, paths, and control text from display
    /// fields, including a nested job.
    pub fn sanitize_secrets(&mut self) {
        self.configured_route =
            displayable_configured_route(self.configured_route.as_deref()).map(str::to_owned);
        self.skills.route_id =
            displayable_configured_route(self.skills.route_id.as_deref()).map(str::to_owned);
        self.agents.route_id =
            displayable_configured_route(self.agents.route_id.as_deref()).map(str::to_owned);
        if let Some(job) = &mut self.job {
            job.sanitize_secrets();
        }
    }
}

impl PrimeIndexUpdate {
    /// Job snapshot with display fields already sanitized.
    pub fn sanitized_job(&self) -> Option<PrimeIndexJobStatus> {
        self.job.clone().map(|mut job| {
            job.sanitize_secrets();
            job
        })
    }
}

/// Capability-aware notification payload. Version-tolerant extras ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrimeIndexUpdate {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<u32>,
    /// Inventory identity hash (blake3 of item id+content_hash pairs), not a
    /// monotonic clock. Restages can produce a numerically smaller value.
    #[serde(default)]
    pub generation: u64,
    /// Monotonic delivery token. Job ticks reuse inventory `generation`, so
    /// pagers and file-poll cursors must key off this sequence, not generation.
    #[serde(default)]
    pub notify_seq: u64,
    #[serde(default)]
    pub fingerprint_short: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<PrimeIndexJobStatus>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_fields: Vec<String>,
}

impl PrimeIndexUpdate {
    /// Inventory `generation` is a blake3 identity, not a watermark.
    ///
    /// When `notify_seq > 0`, delivery order is solely `notifySeq` — a smaller
    /// hash is not stale. Legacy shells (`notify_seq == 0`) still reject
    /// `generation < last_generation`.
    pub fn generation_is_stale_vs(&self, last_generation: u64) -> bool {
        self.notify_seq == 0 && self.generation < last_generation
    }

    pub fn job_is_terminal(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(|job| prime_job_state_is_terminal(&job.state))
    }
}

pub(crate) fn prime_job_state_is_terminal(state: &str) -> bool {
    matches!(state, "completed" | "failed" | "cancelled")
}

pub(crate) fn prime_job_state_is_busy(state: &str) -> bool {
    matches!(state, "running" | "cancelling")
}

/// Request for status/backfill/rebuild/cancel.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimeIndexOpRequest {
    #[serde(default)]
    pub api_version: Option<u32>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub expected_generation: Option<u64>,
    #[serde(default)]
    pub expected_fingerprint: Option<String>,
    /// `"skills"` | `"agents"` | `"all"`. Default `"all"` for status, required
    /// for mutating ops when omitted → `"all"`.
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub confirm_configured_profile: bool,
    /// Live job id; required to cancel a running occupant.
    #[serde(default)]
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimeIndexOpKind {
    Backfill,
    Rebuild,
}

impl PrimeIndexOpKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Backfill => "backfill",
            Self::Rebuild => "rebuild",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VersionError {
    Missing,
    Unsupported,
}

impl VersionError {
    fn message(self) -> &'static str {
        match self {
            Self::Missing => "apiVersion is required for this prime index operation.",
            Self::Unsupported => "Unsupported prime index apiVersion.",
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::Missing => "version_missing",
            Self::Unsupported => "version_unsupported",
        }
    }
}

pub fn require_api_version(api_version: Option<u32>) -> Result<u32, &'static str> {
    match require_version(api_version) {
        Ok(v) => Ok(v),
        Err(e) => Err(e.message()),
    }
}

fn require_version(api_version: Option<u32>) -> Result<u32, VersionError> {
    match api_version {
        Some(PRIME_INDEX_API_VERSION) => Ok(PRIME_INDEX_API_VERSION),
        Some(_) => Err(VersionError::Unsupported),
        None => Err(VersionError::Missing),
    }
}

pub fn fingerprint_short(hash: &str) -> String {
    hash.chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(FINGERPRINT_SHORT_LEN)
        .collect()
}

fn bound_failure(code: &str) -> String {
    code.chars().take(MAX_FAILURE_LEN).collect()
}

/// Profile id only — never an endpoint, path, credential, or control text.
pub fn displayable_configured_route(raw: Option<&str>) -> Option<&str> {
    let route = raw.map(str::trim).filter(|s| !s.is_empty())?;
    if route.chars().count() > MAX_ROUTE_DISPLAY_LEN {
        return None;
    }
    if route.contains("://") || route.contains("sk-") {
        return None;
    }
    let ok = route
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && route.chars().any(|c| c.is_ascii_alphabetic());
    ok.then_some(route)
}

fn is_safe_failure_code(s: &str) -> bool {
    let n = s.chars().count();
    n > 0
        && n <= MAX_FAILURE_LEN
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && s.chars().any(|c| c.is_ascii_alphabetic())
}

/// Isolate a `confirm_required` / `confirm_required:<id>` token from mixed-version
/// ACP errors such as `couldn't run prime index: confirm_required:main`.
fn extract_confirm_required_token(raw: &str) -> Option<&str> {
    let idx = raw.find(PRIME_FAILURE_CONFIRM_REQUIRED)?;
    if idx > 0 {
        let prev = raw[..idx].chars().next_back()?;
        if prev.is_ascii_alphanumeric() || prev == '_' {
            return None;
        }
    }
    let from = raw[idx..].trim_start();
    let token = from
        .split(|c: char| c.is_whitespace() || c.is_control())
        .next()
        .filter(|t| !t.is_empty())?;
    let token = token.trim_end_matches(|c: char| matches!(c, '.' | ',' | ';' | ')' | '"' | '\''));
    if token == PRIME_FAILURE_CONFIRM_REQUIRED || token.starts_with("confirm_required:") {
        Some(token)
    } else {
        None
    }
}

/// True when `failure` is the confirm-required reason (bare code, `:id` suffix,
/// or a mixed-version ACP error that embeds that token).
pub fn prime_failure_is_confirm_required(failure: Option<&str>) -> bool {
    failure.is_some_and(|f| extract_confirm_required_token(f).is_some())
}

/// Safe profile id from a confirm-required failure, if one exists.
/// The remainder after `confirm_required:` must be a single displayable token;
/// control text or extra payload after the id is rejected.
pub fn confirm_required_display_route(raw: &str) -> Option<&str> {
    let idx = raw.find(PRIME_FAILURE_CONFIRM_REQUIRED)?;
    if idx > 0 {
        let prev = raw[..idx].chars().next_back()?;
        if prev.is_ascii_alphanumeric() || prev == '_' {
            return None;
        }
    }
    let from = &raw[idx + PRIME_FAILURE_CONFIRM_REQUIRED.len()..];
    let rest = from.strip_prefix(':')?.trim();
    if rest.is_empty() || rest.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let rest = rest.trim_end_matches(|c: char| matches!(c, '.' | ',' | ';' | ')' | '"' | '\''));
    displayable_configured_route(Some(rest))
}

/// Rewrite a job `failure` to a bounded allowlisted code. Unsanitary
/// `confirm_required:<route>` becomes the stable `confirm_required` code.
/// Prefixed mixed-version ACP errors are reduced to the same codes; unknown
/// prose, paths, endpoints, and overlong payloads become `failed`.
pub fn sanitize_prime_job_failure(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if extract_confirm_required_token(trimmed).is_some() {
        return match confirm_required_display_route(trimmed) {
            Some(id) => bound_failure(&format!("{PRIME_FAILURE_CONFIRM_REQUIRED}:{id}")),
            None => PRIME_FAILURE_CONFIRM_REQUIRED.to_owned(),
        };
    }
    bound_failure(match trimmed {
        "unavailable" | "already_running" | "stale" | "read_only" | "space_mismatch"
        | "embed_failed" | "invalid_item" | "unsupported" | "cancelled" | "failed" => trimmed,
        other if is_safe_failure_code(other) => other,
        _ => "failed",
    })
}

fn map_index_error(err: PrimeIndexError) -> String {
    bound_failure(match err {
        PrimeIndexError::ReadOnly => "read_only",
        PrimeIndexError::SpaceMismatch => "space_mismatch",
        PrimeIndexError::StaleGeneration => "stale",
        PrimeIndexError::EmbedFailed => "embed_failed",
        PrimeIndexError::StagingIncomplete => "staging_incomplete",
        PrimeIndexError::InvalidItem => "invalid_item",
        PrimeIndexError::Unavailable => "unavailable",
    })
}

fn parse_collection(raw: Option<&str>) -> Vec<CollectionKind> {
    match raw.unwrap_or("all") {
        "skills" => vec![CollectionKind::Skills],
        "agents" | "callable_agents" => vec![CollectionKind::CallableAgents],
        _ => vec![CollectionKind::Skills, CollectionKind::CallableAgents],
    }
}

fn collection_wire(kind: CollectionKind) -> &'static str {
    match kind {
        CollectionKind::Skills => "skills",
        CollectionKind::CallableAgents => "agents",
    }
}

fn job_collection_label(kinds: &[CollectionKind]) -> String {
    match kinds {
        [CollectionKind::Skills] => "skills".into(),
        [CollectionKind::CallableAgents] => "agents".into(),
        _ => "all".into(),
    }
}

struct LiveJob {
    status: PrimeIndexJobStatus,
    cancel: CancellationToken,
}

fn jobs() -> &'static Mutex<HashMap<String, LiveJob>> {
    static JOBS: OnceLock<Mutex<HashMap<String, LiveJob>>> = OnceLock::new();
    JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

static JOB_SEQ: AtomicU64 = AtomicU64::new(1);

fn job_key(home: &Path, cwd: &Path) -> String {
    format!(
        "{}::{}",
        home.to_string_lossy(),
        xai_grok_memory::workspace_storage_identity(cwd)
    )
}

fn next_job_id() -> String {
    let n = JOB_SEQ.fetch_add(1, Ordering::Relaxed).saturating_add(1);
    format!("j{n:x}")
}

fn with_jobs<R>(f: impl FnOnce(&mut HashMap<String, LiveJob>) -> R) -> R {
    let mut guard = jobs().lock().unwrap_or_else(|p| p.into_inner());
    f(&mut guard)
}

fn configured_route(home: &Path, collection: Option<CollectionKind>) -> Option<String> {
    let registry = registry_for_home(home)?;
    let snap = registry.load();
    match collection {
        Some(CollectionKind::CallableAgents) => snap.prime.agents.retrieval_profile.clone(),
        Some(CollectionKind::Skills) | None => snap
            .prime
            .skills
            .retrieval_profile
            .clone()
            .or_else(|| snap.prime.agents.retrieval_profile.clone()),
    }
}

fn collection_status_from_handle(
    handle: &PrimeIndexHandle,
    kind: CollectionKind,
    route_id: Option<String>,
) -> PrimeIndexCollectionStatus {
    match handle.collection_snapshot(kind) {
        Ok((state, writable, vec_ok)) => {
            let missing = state.item_count.saturating_sub(state.vec_count).max(0) as u64;
            let readiness = if !writable {
                "read_only"
            } else if state.item_count > 0 && !vec_ok {
                "unavailable"
            } else if missing > 0 {
                "pending"
            } else {
                "ready"
            };
            PrimeIndexCollectionStatus {
                collection: collection_wire(kind).to_owned(),
                generation: state.inventory_generation.max(0) as u64,
                fingerprint_short: fingerprint_short(&state.fingerprint_hash),
                item_count: state.item_count.max(0) as u64,
                vector_count: state.vec_count.max(0) as u64,
                missing_vectors: missing,
                readiness: readiness.to_owned(),
                route_id,
                dimensions: (state.embedding_dimensions > 0)
                    .then_some(state.embedding_dimensions as u32),
            }
        }
        Err(_) => PrimeIndexCollectionStatus {
            collection: collection_wire(kind).to_owned(),
            generation: 0,
            fingerprint_short: String::new(),
            item_count: 0,
            vector_count: 0,
            missing_vectors: 0,
            readiness: "unavailable".into(),
            route_id,
            dimensions: None,
        },
    }
}

fn live_job_for(key: &str) -> Option<PrimeIndexJobStatus> {
    with_jobs(|map| {
        map.get(key).map(|j| {
            let mut job = j.status.clone();
            job.sanitize_secrets();
            job
        })
    })
}

/// Build a status snapshot. Never contacts an embedding provider.
pub fn collect_status(home: &Path, cwd: &Path) -> PrimeIndexStatus {
    let handle = prime_index_for(home, cwd);
    let pin = handle.pinned_space();
    let route = pin.as_ref().map(|p| p.route_id.clone());
    let skills = collection_status_from_handle(
        &handle,
        CollectionKind::Skills,
        route
            .clone()
            .or_else(|| configured_route(home, Some(CollectionKind::Skills))),
    );
    let agents = collection_status_from_handle(
        &handle,
        CollectionKind::CallableAgents,
        route.or_else(|| configured_route(home, Some(CollectionKind::CallableAgents))),
    );
    let generation = skills.generation.max(agents.generation);
    let fingerprint_short = if !skills.fingerprint_short.is_empty() {
        skills.fingerprint_short.clone()
    } else {
        agents.fingerprint_short.clone()
    };
    let key = job_key(home, cwd);
    let mut status = PrimeIndexStatus {
        api_version: PRIME_INDEX_API_VERSION,
        generation,
        fingerprint_short,
        skills,
        agents,
        job: live_job_for(&key),
        configured_route: configured_route(home, None),
        capabilities: PrimeIndexCapabilities::SUPPORTED,
        unchanged: false,
    };
    status.sanitize_secrets();
    status
}

fn stale_mismatch(req: &PrimeIndexOpRequest, status: &PrimeIndexStatus) -> bool {
    if let Some(expected) = req.expected_generation
        && expected != 0
        && expected != status.generation
    {
        return true;
    }
    if let Some(fp) = req.expected_fingerprint.as_deref()
        && !fp.is_empty()
        && fp != status.fingerprint_short
    {
        return true;
    }
    false
}

/// Status refresh. Preconditions matching current generation/fingerprint
/// return `unchanged = true` so TUI search/filter/selection can stay put.
pub fn status(
    home: &Path,
    cwd: &Path,
    req: &PrimeIndexOpRequest,
) -> Result<PrimeIndexStatus, String> {
    require_api_version(req.api_version).map_err(|m| m.to_string())?;
    let mut status = collect_status(home, cwd);
    if let Some(expected) = req.expected_generation
        && expected != 0
        && expected == status.generation
        && req
            .expected_fingerprint
            .as_deref()
            .is_none_or(|fp| fp.is_empty() || fp == status.fingerprint_short)
    {
        status.unchanged = true;
    }
    Ok(status)
}

fn confirm_required_error(route: &str) -> String {
    match displayable_configured_route(Some(route)) {
        Some(id) => format!("{PRIME_FAILURE_CONFIRM_REQUIRED}:{id}"),
        None => PRIME_FAILURE_CONFIRM_REQUIRED.to_owned(),
    }
}

fn needs_confirm(home: &Path, kinds: &[CollectionKind]) -> Option<String> {
    for kind in kinds {
        if let Some(route) = configured_route(home, Some(*kind)) {
            return Some(route);
        }
    }
    configured_route(home, None)
}

fn update_job<F: FnOnce(&mut PrimeIndexJobStatus)>(key: &str, job_id: &str, f: F) -> bool {
    with_jobs(|map| {
        if let Some(job) = map.get_mut(key)
            && job.status.job_id == job_id
        {
            f(&mut job.status);
            return true;
        }
        false
    })
}

fn publish_from_job(home: &Path, key: &str) {
    let (generation, fp, job) = with_jobs(|map| {
        map.get(key)
            .map(|j| {
                let mut job = j.status.clone();
                job.sanitize_secrets();
                (job.generation, job.fingerprint_short.clone(), Some(job))
            })
            .unwrap_or((0, String::new(), None))
    });
    notify::publish_job_update(home, generation, &fp, job, &["job"]);
}

/// Start missing-only backfill or a full rebuild. Returns the running job
/// immediately; work continues in the background.
pub fn start_job(
    home: &Path,
    cwd: &Path,
    req: PrimeIndexOpRequest,
    kind: PrimeIndexOpKind,
) -> Result<PrimeIndexJobStatus, String> {
    require_api_version(req.api_version).map_err(|m| m.to_string())?;
    let status = collect_status(home, cwd);
    if stale_mismatch(&req, &status) {
        return Err("stale".into());
    }
    let kinds = parse_collection(req.collection.as_deref());
    let route = needs_confirm(home, &kinds);
    if let Some(ref route) = route
        && !req.confirm_configured_profile
    {
        // Do not start work. Return a job payload so the pager can show the
        // configured route and require an explicit confirm — never an endpoint.
        let mut job = PrimeIndexJobStatus {
            api_version: PRIME_INDEX_API_VERSION,
            job_id: String::new(),
            kind: kind.as_str().to_owned(),
            collection: job_collection_label(&kinds),
            state: "failed".into(),
            generation: status.generation,
            fingerprint_short: status.fingerprint_short.clone(),
            done: 0,
            total: 0,
            confirm_configured_profile: false,
            configured_route: Some(route.clone()),
            failure: Some(confirm_required_error(route)),
        };
        job.sanitize_secrets();
        return Ok(job);
    }
    let Some(route) = route else {
        return Err("unavailable".into());
    };
    let key = job_key(home, cwd);
    let job_id = next_job_id();
    let cancel = CancellationToken::new();
    let mut job = PrimeIndexJobStatus {
        api_version: PRIME_INDEX_API_VERSION,
        job_id: job_id.clone(),
        kind: kind.as_str().to_owned(),
        collection: job_collection_label(&kinds),
        state: "running".into(),
        generation: status.generation,
        fingerprint_short: status.fingerprint_short.clone(),
        done: 0,
        total: match kinds.as_slice() {
            [CollectionKind::Skills] => status.skills.item_count,
            [CollectionKind::CallableAgents] => status.agents.item_count,
            _ => status
                .skills
                .item_count
                .saturating_add(status.agents.item_count),
        },
        confirm_configured_profile: req.confirm_configured_profile,
        configured_route: Some(route.clone()),
        failure: None,
    };
    job.sanitize_secrets();
    with_jobs(|map| {
        if map
            .get(&key)
            .is_some_and(|j| prime_job_state_is_busy(&j.status.state))
        {
            return Err("already_running");
        }
        map.insert(
            key.clone(),
            LiveJob {
                status: job.clone(),
                cancel: cancel.clone(),
            },
        );
        Ok(())
    })?;
    notify::publish_job_update(
        home,
        job.generation,
        &job.fingerprint_short,
        Some(job.clone()),
        &["job"],
    );

    let home_buf = home.to_path_buf();
    let cwd_buf = cwd.to_path_buf();
    let key_spawn = key.clone();
    let job_id_spawn = job.job_id.clone();
    let cancel_spawn = cancel.clone();
    let kinds_spawn = kinds;
    let kind_spawn = kind;
    let route_spawn = route;
    tokio::spawn(async move {
        run_job(
            home_buf,
            cwd_buf,
            key_spawn,
            job_id_spawn,
            kinds_spawn,
            kind_spawn,
            route_spawn,
            cancel_spawn,
        )
        .await;
    });
    Ok(job)
}

async fn run_job(
    home: PathBuf,
    cwd: PathBuf,
    key: String,
    job_id: String,
    kinds: Vec<CollectionKind>,
    kind: PrimeIndexOpKind,
    route: String,
    cancel: CancellationToken,
) {
    let handle = prime_index_for(&home, &cwd);
    let Some(registry) = registry_for_home(&home) else {
        fail_job(&home, &key, &job_id, "unavailable");
        return;
    };
    let service = registry.service();
    if handle.pin_from_service(&service, &route).is_err() {
        fail_job(&home, &key, &job_id, "unavailable");
        return;
    }
    let mut written_total = 0u64;
    for collection in kinds {
        if cancel.is_cancelled() {
            finish_job(&home, &key, &job_id, "cancelled", None);
            return;
        }
        let frozen = match handle.freeze_pin_for(collection) {
            Ok(f) => f,
            Err(err) => {
                fail_job(&home, &key, &job_id, &map_index_error(err));
                return;
            }
        };
        let embedder = Arc::new(PinnedServiceEmbedder::with_frozen_pin(
            handle.clone(),
            service.clone(),
            route.clone(),
            frozen.clone(),
            cancel.clone(),
        )) as Arc<dyn EmbeddingProvider>;
        let home_progress = home.clone();
        let key_progress = key.clone();
        let job_id_progress = job_id.clone();
        let mut on_progress = move |done: u64, total: u64| {
            if update_job(&key_progress, &job_id_progress, |s| {
                s.done = done;
                if total > 0 {
                    s.total = total;
                }
            }) {
                publish_from_job(&home_progress, &key_progress);
            }
        };
        let result = match kind {
            PrimeIndexOpKind::Backfill => {
                handle
                    .backfill_with_progress(embedder, frozen, cancel.clone(), &mut on_progress)
                    .await
            }
            PrimeIndexOpKind::Rebuild => {
                handle
                    .rebuild_with_progress(embedder, frozen, cancel.clone(), &mut on_progress)
                    .await
            }
        };
        match result {
            Ok(n) => {
                if cancel.is_cancelled() {
                    finish_job(&home, &key, &job_id, "cancelled", None);
                    return;
                }
                written_total = written_total.saturating_add(n as u64);
            }
            Err(PrimeIndexError::Unavailable) if cancel.is_cancelled() => {
                finish_job(&home, &key, &job_id, "cancelled", None);
                return;
            }
            Err(err) => {
                fail_job(&home, &key, &job_id, &map_index_error(err));
                return;
            }
        }
    }
    if cancel.is_cancelled() {
        finish_job(&home, &key, &job_id, "cancelled", None);
        return;
    }
    update_job(&key, &job_id, |s| {
        s.done = written_total.max(s.done);
    });
    finish_job(&home, &key, &job_id, "completed", None);
}

fn fail_job(home: &Path, key: &str, job_id: &str, failure: &str) {
    finish_job(home, key, job_id, "failed", Some(failure));
}

fn finish_job(home: &Path, key: &str, job_id: &str, state: &str, failure: Option<&str>) {
    if !update_job(key, job_id, |s| {
        s.state = state.to_owned();
        s.failure = failure.map(sanitize_prime_job_failure);
        s.sanitize_secrets();
    }) {
        return;
    }
    publish_from_job(home, key);
    // Keep the last job for status until the next start replaces it, but drop
    // the cancel token so idle cancel is a no-op.
    with_jobs(|map| {
        if let Some(job) = map.get_mut(key)
            && job.status.job_id == job_id
        {
            job.cancel = CancellationToken::new();
        }
    });
}

/// Cancel a running job. Idle cancel is a no-op.
pub fn cancel_job(
    home: &Path,
    cwd: &Path,
    req: &PrimeIndexOpRequest,
) -> Result<PrimeIndexJobStatus, String> {
    require_api_version(req.api_version).map_err(|m| m.to_string())?;
    let status = collect_status(home, cwd);
    if stale_mismatch(req, &status) {
        return Err("stale".into());
    }
    let key = job_key(home, cwd);
    let mut found = None;
    let mut stale_job = false;
    with_jobs(|map| {
        if let Some(job) = map.get_mut(&key)
            && job.status.state == "running"
        {
            let expected_id = req.job_id.as_deref().filter(|id| !id.is_empty());
            match expected_id {
                Some(id) if id == job.status.job_id => {
                    job.cancel.cancel();
                    job.status.state = "cancelling".into();
                    found = Some(job.status.clone());
                }
                _ => {
                    stale_job = true;
                    found = Some(job.status.clone());
                }
            }
        } else if let Some(job) = map.get(&key) {
            found = Some(job.status.clone());
        }
    });
    if stale_job {
        return Err("stale".into());
    }
    if let Some(mut job) = found {
        job.sanitize_secrets();
        notify::publish_job_update(
            home,
            job.generation,
            &job.fingerprint_short,
            Some(job.clone()),
            &["job"],
        );
        return Ok(job);
    }
    let status = collect_status(home, cwd);
    Ok(PrimeIndexJobStatus {
        api_version: PRIME_INDEX_API_VERSION,
        job_id: String::new(),
        kind: "backfill".into(),
        collection: req.collection.clone().unwrap_or_else(|| "all".into()),
        state: "cancelled".into(),
        generation: status.generation,
        fingerprint_short: status.fingerprint_short,
        done: 0,
        total: 0,
        confirm_configured_profile: false,
        configured_route: status.configured_route,
        failure: None,
    })
}

/// Initialize-meta advertisement so old pagers ignore unknown fields and new
/// pagers hide controls when talking to an old shell.
pub fn initialize_capability_value() -> serde_json::Value {
    serde_json::json!({
        "apiVersion": PRIME_INDEX_API_VERSION,
        "status": true,
        "backfill": true,
        "rebuild": true,
        "cancel": true,
        "notifications": ["x.ai/prime/index/update"],
        // Skills inventory/validation/regression remain on the skills ACP
        // surface (apiVersion 1) — not duplicated here.
        "skillsApiVersion": SKILLS_API_VERSION,
    })
}

pub fn parse_prime_index_available(meta: Option<&serde_json::Value>) -> PrimeIndexCapabilities {
    let Some(v) = meta.and_then(|m| m.get("primeIndex")) else {
        return PrimeIndexCapabilities::UNSUPPORTED;
    };
    PrimeIndexCapabilities {
        status: v.get("status").and_then(|x| x.as_bool()).unwrap_or(false),
        backfill: v.get("backfill").and_then(|x| x.as_bool()).unwrap_or(false),
        rebuild: v.get("rebuild").and_then(|x| x.as_bool()).unwrap_or(false),
        cancel: v.get("cancel").and_then(|x| x.as_bool()).unwrap_or(false),
    }
}

/// Compact inspect projection. Truncated fingerprints only. Does not create
/// a new index when none exists.
pub fn inspect_status(home: &Path, cwd: Option<&Path>) -> Option<PrimeIndexStatus> {
    let cwd = cwd?;
    let identity = xai_grok_memory::workspace_storage_identity(cwd);
    let db = xai_grok_memory::metadata_index_path(home, &identity);
    if !db.exists() {
        return None;
    }
    Some(collect_status(home, cwd))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;
    use xai_grok_memory::embedding::MockEmbeddingProvider;
    use xai_grok_memory::{EmbeddingSourceSpec, NORMALIZATION_L2_V1};

    use crate::session::prime::index::{
        PinnedEmbeddingSpace, skills_to_index_items, uninstall_prime_index,
    };
    use xai_grok_tools::implementations::skills::types::{SkillInfo, SkillScope};

    fn skill(name: &str, path: &str) -> SkillInfo {
        SkillInfo {
            name: name.into(),
            path: path.into(),
            description: format!("{name} description for indexing"),
            has_user_specified_description: true,
            when_to_use: Some(format!("use {name} when relevant")),
            paths: Some(vec!["src/**".into()]),
            scope: SkillScope::Repo,
            enabled: true,
            ..SkillInfo::default()
        }
    }

    fn pin_mock(handle: &PrimeIndexHandle) {
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-1".into(),
                space_fingerprint: "fp".into(),
                spec: EmbeddingSourceSpec {
                    provider_instance_id: "p".into(),
                    incarnation: None,
                    origin_host: "example.test".into(),
                    embedding_path: "/v1/embeddings".into(),
                    protocol: "openai_compatible".into(),
                    model: "mock".into(),
                    dimensions: 4,
                    encoding: "float".into(),
                    normalization: NORMALIZATION_L2_V1.to_owned(),
                },
            })
            .unwrap();
    }

    #[test]
    fn version_missing_and_unsupported_fail_closed() {
        assert_eq!(
            require_api_version(None).unwrap_err(),
            VersionError::Missing.message()
        );
        assert_eq!(
            require_api_version(Some(0)).unwrap_err(),
            VersionError::Unsupported.message()
        );
        assert_eq!(require_api_version(Some(1)).unwrap(), 1);
        assert_eq!(VersionError::Missing.code(), "version_missing");
    }

    #[test]
    fn fingerprint_short_is_truncated_hex_only() {
        assert_eq!(fingerprint_short("abcdef0123456789ffff"), "abcdef012345");
        assert_eq!(fingerprint_short(""), "");
        assert_eq!(fingerprint_short("zzz"), "");
    }

    #[test]
    fn status_and_job_payloads_are_secret_free() {
        let status = PrimeIndexStatus {
            api_version: 1,
            generation: 3,
            fingerprint_short: "abc123def456".into(),
            skills: PrimeIndexCollectionStatus {
                collection: "skills".into(),
                generation: 3,
                fingerprint_short: "abc123def456".into(),
                item_count: 2,
                vector_count: 1,
                missing_vectors: 1,
                readiness: "pending".into(),
                route_id: Some("main".into()),
                dimensions: Some(8),
            },
            agents: PrimeIndexCollectionStatus {
                collection: "agents".into(),
                generation: 1,
                fingerprint_short: String::new(),
                item_count: 0,
                vector_count: 0,
                missing_vectors: 0,
                readiness: "ready".into(),
                route_id: None,
                dimensions: None,
            },
            job: Some(PrimeIndexJobStatus {
                api_version: 1,
                job_id: "j1".into(),
                kind: "backfill".into(),
                collection: "skills".into(),
                state: "failed".into(),
                generation: 3,
                fingerprint_short: "abc123def456".into(),
                done: 0,
                total: 2,
                confirm_configured_profile: false,
                configured_route: Some("main".into()),
                failure: Some("embed_failed".into()),
            }),
            configured_route: Some("main".into()),
            capabilities: PrimeIndexCapabilities::SUPPORTED,
            unchanged: false,
        };
        let json = serde_json::to_string(&status).unwrap();
        for leak in [
            "sk-",
            "/Users/",
            "http://",
            "prompt",
            "BODY",
            "vector_values",
            "abcdef0123456789ffff",
        ] {
            assert!(!json.contains(leak), "leaked {leak} in {json}");
        }
        assert!(json.contains("fingerprintShort"));
        assert!(!json.contains("\"fingerprint\""));
        let update: PrimeIndexUpdate = serde_json::from_str("{}").unwrap();
        assert_eq!(update.generation, 0);
        assert_eq!(update.notify_seq, 0);
        assert!(update.job.is_none());
    }

    #[test]
    fn old_pager_ignores_unknown_status_fields() {
        let json = r#"{"apiVersion":1,"generation":2,"fingerprintShort":"aa","skills":{"collection":"skills","generation":2,"fingerprintShort":"aa","itemCount":0,"vectorCount":0,"missingVectors":0,"readiness":"ready"},"agents":{"collection":"agents","generation":0,"fingerprintShort":"","itemCount":0,"vectorCount":0,"missingVectors":0,"readiness":"ready"},"capabilities":{"status":true,"backfill":true,"rebuild":true,"cancel":true},"futureField":true}"#;
        let parsed: PrimeIndexStatus = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.generation, 2);
        assert!(parsed.job.is_none());
    }

    #[test]
    fn initialize_capability_defaults_off_when_absent() {
        assert_eq!(
            parse_prime_index_available(None),
            PrimeIndexCapabilities::UNSUPPORTED
        );
        let meta = serde_json::json!({"sessionRecap": true});
        assert_eq!(
            parse_prime_index_available(Some(&meta)),
            PrimeIndexCapabilities::UNSUPPORTED
        );
        let meta = serde_json::json!({"primeIndex": initialize_capability_value()});
        let caps = parse_prime_index_available(Some(&meta));
        assert!(caps.status && caps.backfill && caps.rebuild && caps.cancel);
    }

    #[test]
    fn stale_precondition_is_detected() {
        let status = PrimeIndexStatus {
            api_version: 1,
            generation: 4,
            fingerprint_short: "abcd".into(),
            skills: PrimeIndexCollectionStatus {
                collection: "skills".into(),
                generation: 4,
                fingerprint_short: "abcd".into(),
                item_count: 0,
                vector_count: 0,
                missing_vectors: 0,
                readiness: "ready".into(),
                route_id: None,
                dimensions: None,
            },
            agents: PrimeIndexCollectionStatus {
                collection: "agents".into(),
                generation: 0,
                fingerprint_short: String::new(),
                item_count: 0,
                vector_count: 0,
                missing_vectors: 0,
                readiness: "ready".into(),
                route_id: None,
                dimensions: None,
            },
            job: None,
            configured_route: None,
            capabilities: PrimeIndexCapabilities::SUPPORTED,
            unchanged: false,
        };
        let stale_gen = PrimeIndexOpRequest {
            expected_generation: Some(3),
            ..Default::default()
        };
        assert!(stale_mismatch(&stale_gen, &status));
        let stale_fp = PrimeIndexOpRequest {
            expected_generation: Some(4),
            expected_fingerprint: Some("ffff".into()),
            ..Default::default()
        };
        assert!(stale_mismatch(&stale_fp, &status));
        let ok = PrimeIndexOpRequest {
            expected_generation: Some(4),
            expected_fingerprint: Some("abcd".into()),
            ..Default::default()
        };
        assert!(!stale_mismatch(&ok, &status));
    }

    #[tokio::test]
    async fn missing_only_backfill_does_not_require_full_restage() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = prime_index_for(&home, &cwd);
        let items = skills_to_index_items(&[skill("a", "skills/a/SKILL.md")]);
        handle.sync_skills(1, &items).unwrap();
        pin_mock(&handle);
        let frozen = handle.freeze_pin().unwrap();
        let embedder = Arc::new(MockEmbeddingProvider { dimensions: 4 });
        let n = handle
            .backfill(embedder.clone(), frozen.clone(), CancellationToken::new())
            .await
            .expect("backfill");
        let before = handle.collection_snapshot(CollectionKind::Skills).unwrap();
        assert!(
            before.0.item_count >= 1,
            "inventory must persist before missing-only fill"
        );
        if before.2 && before.1 {
            assert!(
                n >= 1 || before.0.vec_count >= 1,
                "writable vec-capable index must fill or already have vectors"
            );
        }
        let n2 = handle
            .backfill(embedder, frozen, CancellationToken::new())
            .await
            .expect("second missing-only");
        assert_eq!(n2, 0, "second missing-only pass writes nothing");
        let after = handle.collection_snapshot(CollectionKind::Skills).unwrap();
        assert_eq!(before.0.vec_count, after.0.vec_count);
        uninstall_prime_index(&home, &cwd);
    }

    #[tokio::test]
    async fn cancel_idle_is_noop_and_version_is_required() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let err = cancel_job(
            &home,
            &cwd,
            &PrimeIndexOpRequest {
                api_version: None,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(err, VersionError::Missing.message());
        let job = cancel_job(
            &home,
            &cwd,
            &PrimeIndexOpRequest {
                api_version: Some(1),
                cwd: Some(cwd.to_string_lossy().into_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(job.state, "cancelled");
        assert!(job.job_id.is_empty());
        uninstall_prime_index(&home, &cwd);
    }

    #[test]
    fn status_without_index_is_unavailable_not_a_secret() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let req = PrimeIndexOpRequest {
            api_version: Some(1),
            cwd: Some(cwd.to_string_lossy().into_owned()),
            expected_generation: Some(1),
            expected_fingerprint: Some("deadbeef".into()),
            ..Default::default()
        };
        let status = status(&home, &cwd, &req).unwrap();
        assert_eq!(status.api_version, 1);
        assert!(
            !status.unchanged,
            "missing index cannot match a fingerprint"
        );
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains(home.to_string_lossy().as_ref()), "{json}");
        uninstall_prime_index(&home, &cwd);
    }

    #[test]
    fn start_job_without_profile_is_unavailable() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let err = start_job(
            &home,
            &cwd,
            PrimeIndexOpRequest {
                api_version: Some(1),
                cwd: Some(cwd.to_string_lossy().into_owned()),
                confirm_configured_profile: true,
                ..Default::default()
            },
            PrimeIndexOpKind::Backfill,
        )
        .unwrap_err();
        assert_eq!(err, "unavailable");
        uninstall_prime_index(&home, &cwd);
    }

    #[test]
    fn confirm_required_error_includes_route_not_endpoint() {
        let msg = confirm_required_error("main");
        assert_eq!(msg, "confirm_required:main");
        assert!(!msg.contains("http"));
        let job = PrimeIndexJobStatus {
            api_version: 1,
            job_id: String::new(),
            kind: "rebuild".into(),
            collection: "skills".into(),
            state: "failed".into(),
            generation: 1,
            fingerprint_short: "abc123def456".into(),
            done: 0,
            total: 0,
            confirm_configured_profile: false,
            configured_route: Some("main".into()),
            failure: Some(msg),
        };
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("confirm_required:main"), "{json}");
        assert!(!json.contains("http://"), "{json}");
        assert!(!json.contains("/Users/"), "{json}");
    }

    fn unsanitary_job(raw: &str) -> PrimeIndexJobStatus {
        PrimeIndexJobStatus {
            api_version: 1,
            job_id: String::new(),
            kind: "backfill".into(),
            collection: "skills".into(),
            state: "failed".into(),
            generation: 1,
            fingerprint_short: "abc123def456".into(),
            done: 0,
            total: 0,
            confirm_configured_profile: false,
            configured_route: Some(raw.into()),
            failure: Some(format!("confirm_required:{raw}")),
        }
    }

    #[test]
    fn unsanitary_configured_routes_never_appear_on_the_wire() {
        let long = "x".repeat(200);
        let cases = [
            "http://127.0.0.1/v1",
            "sk-live-secret",
            "file:///tmp/secret",
            "main\nsk-live-secret",
            long.as_str(),
        ];
        for raw in cases {
            let mut job = unsanitary_job(raw);
            job.sanitize_secrets();
            assert_eq!(
                job.failure.as_deref(),
                Some(PRIME_FAILURE_CONFIRM_REQUIRED),
                "failure for {raw:?}"
            );
            assert!(
                job.configured_route.is_none(),
                "configured_route must be omitted for {raw:?}"
            );
            let json = serde_json::to_string(&job).unwrap();
            for leak in [raw, raw.trim(), "127.0.0.1", "sk-live-secret", "file://"] {
                if leak.is_empty() {
                    continue;
                }
                assert!(
                    !json.contains(leak),
                    "leaked {leak:?} from {raw:?} in {json}"
                );
            }
            assert!(!json.contains('\n'), "{json}");
            assert!(!json.contains('\u{0007}'), "{json}");
        }
        assert_eq!(
            confirm_required_error("http://127.0.0.1/v1"),
            "confirm_required"
        );
        assert_eq!(confirm_required_error("sk-live-secret"), "confirm_required");
        assert_eq!(displayable_configured_route(Some("main")), Some("main"));
        assert!(displayable_configured_route(Some("http://127.0.0.1/v1")).is_none());
        assert!(prime_failure_is_confirm_required(Some("confirm_required")));
        assert!(prime_failure_is_confirm_required(Some(
            "confirm_required:main"
        )));
        assert!(prime_failure_is_confirm_required(Some(
            "couldn't run prime index: confirm_required:lab-emb"
        )));
        assert!(!prime_failure_is_confirm_required(Some(
            "xconfirm_required:main"
        )));
    }

    #[test]
    fn sanitize_prime_job_failure_strips_prefixed_payloads_and_overlong_prose() {
        assert_eq!(
            sanitize_prime_job_failure("couldn't run prime index: confirm_required:main"),
            "confirm_required:main"
        );
        assert_eq!(
            sanitize_prime_job_failure(
                "couldn't run prime index: confirm_required:http://127.0.0.1/v1"
            ),
            PRIME_FAILURE_CONFIRM_REQUIRED
        );
        assert_eq!(
            sanitize_prime_job_failure("ACP error: confirm_required:sk-live-secret"),
            PRIME_FAILURE_CONFIRM_REQUIRED
        );
        assert_eq!(
            sanitize_prime_job_failure("confirm_required:file:///tmp/secret"),
            PRIME_FAILURE_CONFIRM_REQUIRED
        );
        let long = "x".repeat(200);
        assert_eq!(sanitize_prime_job_failure(&long), "failed");
        assert_eq!(
            sanitize_prime_job_failure("couldn't run prime index: boom"),
            "failed"
        );
        assert_eq!(
            sanitize_prime_job_failure("already_running"),
            "already_running"
        );
        assert_eq!(
            confirm_required_display_route("couldn't run prime index: confirm_required:lab-emb"),
            Some("lab-emb")
        );
        assert_eq!(
            confirm_required_display_route("confirm_required:http://127.0.0.1/v1"),
            None
        );
        assert_eq!(
            confirm_required_display_route("confirm_required:main\nsk-live-secret"),
            None
        );
        assert_eq!(
            sanitize_prime_job_failure("confirm_required:main\nsk-live-secret"),
            PRIME_FAILURE_CONFIRM_REQUIRED
        );
    }

    #[test]
    fn start_job_confirm_required_omits_unsanitary_configured_route() {
        use crate::retrieval::{
            RetrievalRegistry, install_registry_for_home, uninstall_registry_for_home,
        };
        use std::sync::Arc;

        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let raw = "http://127.0.0.1/v1/embeddings?key=sk-live-secret";
        let reg = RetrievalRegistry::disabled(&home);
        let mut snap = (*reg.load()).clone();
        snap.fingerprint = "unsanitary-route".into();
        snap.generation = 1;
        snap.prime.skills.retrieval_profile = Some(raw.into());
        let _ = reg.try_publish(0, Arc::new(snap));
        install_registry_for_home(&home, reg);
        let job = start_job(
            &home,
            &cwd,
            PrimeIndexOpRequest {
                api_version: Some(1),
                cwd: Some(cwd.to_string_lossy().into_owned()),
                confirm_configured_profile: false,
                ..Default::default()
            },
            PrimeIndexOpKind::Backfill,
        )
        .expect("confirm-required job");
        assert_eq!(job.failure.as_deref(), Some(PRIME_FAILURE_CONFIRM_REQUIRED));
        assert!(job.configured_route.is_none());
        let json = serde_json::to_string(&job).unwrap();
        assert!(!json.contains(raw), "{json}");
        assert!(!json.contains("sk-live-secret"), "{json}");
        assert!(!json.contains("127.0.0.1"), "{json}");
        uninstall_registry_for_home(&home);
        uninstall_prime_index(&home, &cwd);
    }

    #[test]
    fn older_v1_job_omitting_additive_fields_deserializes_fail_closed() {
        let json = r#"{"apiVersion":1,"jobId":"j1","kind":"rebuild","collection":"agents","state":"running","generation":3,"fingerprintShort":"abc123def456","done":1,"total":2}"#;
        let job: PrimeIndexJobStatus = serde_json::from_str(json).unwrap();
        assert!(!job.confirm_configured_profile);
        assert!(job.configured_route.is_none());
        assert!(job.failure.is_none());
        assert_eq!(job.kind, "rebuild");
        assert_eq!(job.collection, "agents");
        assert_eq!(job.done, 1);
        assert_eq!(job.total, 2);

        let update_json = format!(
            r#"{{"schemaVersion":1,"apiVersion":1,"generation":3,"notifySeq":9,"fingerprintShort":"abc123def456","job":{json}}}"#
        );
        let update: PrimeIndexUpdate = serde_json::from_str(&update_json).unwrap();
        assert_eq!(update.notify_seq, 9);
        assert_eq!(update.generation, 3);
        let parsed = update
            .job
            .expect("nested job must survive missing additive fields");
        assert!(!parsed.confirm_configured_profile);
        assert_eq!(parsed.kind, "rebuild");
        assert_eq!(parsed.collection, "agents");
    }

    fn sample_live_status(job_id: &str, state: &str, done: u64, total: u64) -> PrimeIndexJobStatus {
        PrimeIndexJobStatus {
            api_version: 1,
            job_id: job_id.into(),
            kind: "backfill".into(),
            collection: "skills".into(),
            state: state.into(),
            generation: 1,
            fingerprint_short: "abc123def456".into(),
            done,
            total,
            confirm_configured_profile: false,
            configured_route: Some("main".into()),
            failure: None,
        }
    }

    #[test]
    fn start_job_treats_cancelling_as_already_running() {
        use crate::retrieval::{
            RetrievalRegistry, install_registry_for_home, uninstall_registry_for_home,
        };
        use std::sync::Arc;

        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let key = job_key(&home, &cwd);

        let reg = RetrievalRegistry::disabled(&home);
        let mut snap = (*reg.load()).clone();
        snap.fingerprint = "cancelling-busy".into();
        snap.generation = 1;
        snap.prime.skills.retrieval_profile = Some("main".into());
        let _ = reg.try_publish(0, Arc::new(snap));
        install_registry_for_home(&home, reg);

        for busy in ["running", "cancelling"] {
            with_jobs(|map| {
                map.insert(
                    key.clone(),
                    LiveJob {
                        status: sample_live_status("j-old", busy, 1, 4),
                        cancel: CancellationToken::new(),
                    },
                );
            });
            let err = start_job(
                &home,
                &cwd,
                PrimeIndexOpRequest {
                    api_version: Some(1),
                    cwd: Some(cwd.to_string_lossy().into_owned()),
                    confirm_configured_profile: true,
                    ..Default::default()
                },
                PrimeIndexOpKind::Backfill,
            )
            .unwrap_err();
            assert_eq!(err, "already_running", "busy state {busy}");
            let occupant = with_jobs(|map| {
                map.get(&key).map(|j| {
                    (
                        j.status.job_id.clone(),
                        j.status.state.clone(),
                        j.status.done,
                    )
                })
            });
            assert_eq!(
                occupant
                    .as_ref()
                    .map(|(id, state, done)| (id.as_str(), state.as_str(), *done)),
                Some(("j-old", busy, 1)),
                "{busy} occupant must not be replaced"
            );
        }

        with_jobs(|map| {
            map.remove(&key);
        });
        uninstall_registry_for_home(&home);
        uninstall_prime_index(&home, &cwd);
    }

    #[test]
    fn finish_and_progress_ignore_stale_job_id() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let key = "stale-job-id-guard";
        with_jobs(|map| {
            map.insert(
                key.to_owned(),
                LiveJob {
                    status: sample_live_status("j-new", "running", 2, 8),
                    cancel: CancellationToken::new(),
                },
            );
        });

        assert!(!update_job(key, "j-old", |s| {
            s.done = 99;
            s.total = 99;
            s.state = "failed".into();
        }));
        finish_job(&home, key, "j-old", "cancelled", None);
        let occupant = with_jobs(|map| map.get(key).map(|j| j.status.clone()));
        let job = occupant.expect("replacement job");
        assert_eq!(job.job_id, "j-new");
        assert_eq!(job.state, "running");
        assert_eq!(job.done, 2);
        assert_eq!(job.total, 8);

        assert!(update_job(key, "j-new", |s| {
            s.done = 3;
        }));
        let updated = with_jobs(|map| map.get(key).map(|j| j.status.done));
        assert_eq!(updated, Some(3));

        with_jobs(|map| {
            map.remove(key);
        });
    }
}
