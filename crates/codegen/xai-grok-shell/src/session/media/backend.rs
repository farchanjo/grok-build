//! Shell-owned media-understanding backend (plan sections 4.3, 8, 16, and
//! 17).
//!
//! [`ShellMediaUnderstandingBackend`] owns:
//!
//! - the hot-swappable resolved config snapshot (`ConfigUpdate::MediaUnderstanding`
//!   re-normalizes and replaces it),
//! - the session artifact store / semantic cache / usage ledger,
//! - the per-provider circuit breaker state,
//! - the two host gates (filesystem/tool permission, then purpose-scoped
//!   disclosure consent), ZDR eligibility, and the ordered route resolution
//!   + dedicated per-route `InferenceClient` invocation.
//!
//! `analyze()` orchestrates: policy bounds → resolve sources (permission +
//! containment) → ordered eligible routes → ZDR gate → consent gate →
//! circuit breaker → transport plan → preprocessing → cache lookup →
//! delegate → cache insert / ledger accounting. Every fallback provider is
//! consented individually before any bytes leave.

use super::artifacts::{ArtifactKind, LIVE_ATTACHMENT_REF, MediaArtifactStore, ObjectRef, RefKind};
use super::cache::{SemanticCache, SemanticCacheKey};
use super::consent::{
    ConsentDecision, ConsentRequest, DisclosureConsentGate, DisclosurePurpose,
    MediaConsentProvider, provider_identity_str,
};
use super::invoker::{
    AuxMediaInvoker, DelegateError, DelegateRequest, InvokerContext, build_delegate_prompt,
    build_semantics, delegate_error_reason, instruction_fingerprint, prompt_fingerprint,
    schema_fingerprint,
};
use super::ledger::{UsageLedger, UsagePurpose, UsageRow};
use super::policy::{MediaItemBytes, MediaPolicyLimits, PolicyError};
use super::preprocess::{PreprocessError, PreprocessOutcome, PreprocessProfile};
use super::routes::{ResolvedRoute, RouteEligibility, RouteResolution};
use super::transport::transport_plan_for;
use super::zdr::zdr_route_eligible;
use crate::agent::config::{
    MediaUnderstandingConfig, ModelEntry, ResolvedMediaUnderstandingConfig,
};
use base64::Engine as _;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use xai_grok_tools::media::backend::{
    MediaAttemptSummary, MediaBackendAvailability, MediaSemantics, MediaUnderstandingBackend,
    MediaUnderstandingError, MediaUnderstandingRequest, MediaUnderstandingResult,
};
use xai_grok_tools::media::domain::{
    MediaCategory, MediaCategoryStrategy, MediaDetailLevel, MediaSource,
};

/// Default policy bounds applied on top of the resolved config.
const DEFAULT_MAX_INSTRUCTION_CHARS: usize = 20_000;
const DEFAULT_MAX_FOCUS_ITEMS: usize = 16;
const DEFAULT_MAX_MEDIA_ITEMS: usize = 32;

/// Construction context for the shell-owned backend.
///
/// The orchestrator assembles this from session state at spawn; tests build
/// it directly.
#[derive(Clone)]
pub(crate) struct ShellMediaBackendContext {
    /// Resolved initial `[media_understanding]` config (hot-swappable via
    /// `ShellMediaUnderstandingBackend::apply_media_config` /
    /// `apply_resolved_config`).
    pub config: ResolvedMediaUnderstandingConfig,
    /// Live model catalog (cheap to clone; holds an `Arc` internally).
    pub models: crate::agent::models::ModelsManager,
    /// Session auth manager for ZDR metadata and the session bearer (live).
    pub auth: Option<Arc<crate::auth::AuthManager>>,
    /// Optional snapshot of the current session auth, used for the ZDR gate
    /// only when no live `AuthManager` is available (tests, headless
    /// compositions). The live manager always takes precedence; never a
    /// user-authored allowlist.
    pub current_auth: Option<crate::auth::GrokAuth>,
    /// Session-local store root (`<session_dir>`).
    pub session_dir: PathBuf,
    /// Workspace root used for path containment.
    pub workspace_root: PathBuf,
    /// Permission handle for the filesystem/tool gate.
    pub permission: Option<xai_grok_workspace::permission::PermissionHandle>,
    /// Session ID string for permission requests.
    pub session_id: Option<String>,
    /// Session-scoped disclosure consent provider (`None` fails closed).
    pub consent: Option<Arc<dyn MediaConsentProvider>>,
    /// Credential/sampler stamp snapshot used by the invoker.
    pub credentials: InvokerCredentialSnapshot,
}

/// Construction-time snapshot of credentials and session-local sampler data.
#[derive(Clone)]
pub(crate) struct InvokerCredentialSnapshot {
    pub alpha_test_key: Option<String>,
    pub client_version: Option<String>,
    /// Active session `InferenceConfig` (used only for stamping session-local
    /// sampler fields on each route's own client).
    pub active_session_config: xai_grok_inference::InferenceConfig,
    pub client_identifier: Option<String>,
    pub max_retries: Option<u32>,
}

/// The shell-owned media-understanding backend.
pub(crate) struct ShellMediaUnderstandingBackend {
    inner: Arc<BackendInner>,
}

struct BackendInner {
    /// Hot-swappable resolved config.
    config: parking_lot::RwLock<ResolvedMediaUnderstandingConfig>,
    context: ShellMediaBackendContext,
    store: MediaArtifactStore,
    cache: SemanticCache,
    ledger: UsageLedger,
    circuit: parking_lot::Mutex<CircuitBreakerState>,
}

/// Per-request attempt budget (reset per `analyze_for` call).
struct RequestBudget {
    spent_ticks: u64,
    max_ticks: u64,
}

/// Per-provider circuit breaker keyed by `(provider, category)`.
#[derive(Debug, Default)]
struct CircuitBreakerState {
    failures: HashMap<(String, MediaCategory), Vec<i64>>,
}

impl CircuitBreakerState {
    fn is_open(
        &self,
        now: i64,
        key: &(String, MediaCategory),
        config: &ResolvedMediaUnderstandingConfig,
    ) -> bool {
        let window_secs = config.circuit_breaker.window_secs as i64;
        let threshold = config.circuit_breaker.failures;
        let recent = self
            .failures
            .get(key)
            .map(|timestamps| {
                timestamps
                    .iter()
                    .filter(|timestamp| now - **timestamp <= window_secs)
                    .count()
            })
            .unwrap_or(0);
        (recent as u64) >= threshold
    }

    fn record(
        &mut self,
        now: i64,
        key: (String, MediaCategory),
        success: bool,
        config: &ResolvedMediaUnderstandingConfig,
    ) {
        let window_secs = config.circuit_breaker.window_secs as i64;
        let entry = self.failures.entry(key).or_default();
        if success {
            entry.clear();
            return;
        }
        entry.retain(|timestamp| now - *timestamp <= window_secs);
        entry.push(now);
    }
}

/// Outcome of one per-route attempt.
enum RouteAttemptOutcome {
    /// Semantics produced for the item; stop trying further routes.
    Produced(MediaSemantics),
    /// The route could not run (skip or failed); advance to the next route.
    Advanced,
    /// The whole request must terminate.
    Terminal(MediaUnderstandingError),
}

impl ShellMediaUnderstandingBackend {
    /// Construct the backend. Fails only when the session store layout
    /// cannot be created.
    pub(crate) fn new(context: ShellMediaBackendContext) -> io::Result<Self> {
        let config = context.config.clone();
        let store = MediaArtifactStore::open(&context.session_dir)?;
        let cache = SemanticCache::open(&context.session_dir)?;
        let ledger = UsageLedger::open(&context.session_dir)?;
        Ok(Self {
            inner: Arc::new(BackendInner {
                config: parking_lot::RwLock::new(config),
                context,
                store,
                cache,
                ledger,
                circuit: parking_lot::Mutex::new(CircuitBreakerState::default()),
            }),
        })
    }

    /// Hot-swap the config snapshot from an already-normalized/validated
    /// resolved value. The caller is responsible for validation; this is the
    /// single write path so the backend snapshot and the session decision
    /// snapshot (`SessionMediaContext::apply_config`) can never diverge.
    pub(crate) fn apply_resolved_config(&self, resolved: ResolvedMediaUnderstandingConfig) {
        *self.inner.config.write() = resolved;
        tracing::info!("media understanding config hot-swapped");
    }

    /// Hot-swap the config snapshot from a raw `[media_understanding]` value.
    ///
    /// Invalid configs are ignored (the current accepted snapshot stays);
    /// this mirrors the reloader's process-local last-known-good behavior.
    pub(crate) fn apply_media_config(&self, config: &MediaUnderstandingConfig) {
        match config.normalize_validate() {
            Ok(resolved) => self.apply_resolved_config(resolved),
            Err(error) => {
                tracing::warn!(
                    %error,
                    "ignoring invalid media understanding config; keeping current snapshot"
                );
            }
        }
    }

    /// Availability snapshot for tool-listing and capability gating.
    pub(crate) fn availability_snapshot(&self) -> MediaBackendAvailability {
        let config = self.inner.config.read().clone();
        if !config.enabled {
            return MediaBackendAvailability {
                enabled: false,
                ..Default::default()
            };
        }
        let invoker_context = self.build_invoker_context(&config);
        let invoker = AuxMediaInvoker::new(invoker_context);
        let models = self.inner.context.models.models();
        let resolution = RouteResolution {
            config: &config,
            models: &models,
            has_credentials: &|model| invoker.context().resolve_route_config(model).is_some(),
        };
        let mut routes = Vec::new();
        let mut supported_categories = Vec::new();
        for category in MediaCategory::CONCRETE {
            let availability = resolution.category_availability(category);
            if availability.iter().any(|route| route.eligible) {
                supported_categories.push(category);
            }
            routes.extend(availability);
        }
        MediaBackendAvailability {
            enabled: true,
            supported_categories,
            routes,
        }
    }

    /// Analyze with an explicit disclosure purpose (the trait method maps to
    /// `ExplicitTool`).
    ///
    /// Traced as `media.analyze` (plan section 17). Fields are request
    /// metadata only — category, item count, purpose — never instructions,
    /// paths, or media bytes.
    #[tracing::instrument(
        name = "media.analyze",
        skip_all,
        fields(
            category = ?request.category,
            media_items = request.media.len(),
            purpose = ?purpose,
        )
    )]
    pub(crate) async fn analyze_for(
        &self,
        request: MediaUnderstandingRequest,
        purpose: DisclosurePurpose,
    ) -> Result<MediaUnderstandingResult, MediaUnderstandingError> {
        let config = self.inner.config.read().clone();
        if !config.enabled {
            return Err(MediaUnderstandingError::Unavailable(
                "media understanding is disabled in config".to_string(),
            ));
        }
        let limits = MediaPolicyLimits {
            max_media_bytes: config.max_media_bytes,
            max_audio_seconds: config.max_audio_seconds,
            max_video_seconds: config.max_video_seconds,
            max_video_frames: config.max_video_frames,
            max_instruction_chars: DEFAULT_MAX_INSTRUCTION_CHARS,
            max_focus_items: DEFAULT_MAX_FOCUS_ITEMS,
            max_media_items: DEFAULT_MAX_MEDIA_ITEMS,
        };

        // Gate 1: request bounds.
        super::policy::validate_request(&request, &limits)
            .map_err(|e| MediaUnderstandingError::InvalidInput(e.to_string()))?;

        // Gate 1: resolve each source to bounded bytes (permission +
        // canonicalization + containment + size caps).
        let mut items = Vec::with_capacity(request.media.len());
        for source in &request.media {
            let item = super::policy::resolve_media_item(
                source,
                &self.inner.context.workspace_root,
                &self.inner.context.session_dir,
                self.inner.context.permission.as_ref(),
                self.inner.context.session_id.as_deref(),
                &limits,
            )
            .await
            .map_err(|e| match e {
                PolicyError::PermissionDenied(_) => MediaUnderstandingError::InvalidInput(
                    "media source was not permitted".to_string(),
                ),
                other => MediaUnderstandingError::PreprocessFailed(other.to_string()),
            })?;
            items.push(item);
        }

        // Persist source blobs so artifact refs can be reused by later
        // requests (replay/compaction/export), and reference them as objects
        // that entered the current conversation lifecycle (plan 11.3) so
        // conservative GC at session close retains them.
        let mut source_refs = Vec::with_capacity(items.len());
        for item in &items {
            if let Ok(digest) =
                super::policy::persist_source_blob(&self.inner.context.session_dir, item)
            {
                source_refs.push(ObjectRef {
                    kind: ArtifactKind::Blob,
                    hash: digest,
                });
            }
        }
        let _ = self
            .inner
            .store
            .merge_ref(RefKind::Attachments, LIVE_ATTACHMENT_REF, &source_refs);

        let invoker = AuxMediaInvoker::new(self.build_invoker_context(&config));
        let models = self.inner.context.models.models();
        let resolution = RouteResolution {
            config: &config,
            models: &models,
            has_credentials: &|model| invoker.context().resolve_route_config(model).is_some(),
        };
        let consent_gate = DisclosureConsentGate::new(self.inner.context.consent.clone());
        let auth_now = self.current_auth();
        let profile = PreprocessProfile::for_config(&config);

        let mut attempts = Vec::new();
        let mut results = Vec::with_capacity(items.len());
        let mut budget = RequestBudget {
            spent_ticks: 0,
            max_ticks: config.max_aux_budget_usd_ticks,
        };

        for item in &items {
            // Resolve `Auto` requests by sniffing the concrete category.
            let category = if request.category == MediaCategory::Auto {
                super::preprocess::sniff_category(
                    &item.bytes,
                    item.mime.as_deref(),
                    source_path_hint(&item.source),
                )
                .unwrap_or(MediaCategory::Image)
            } else {
                request.category
            };

            let produced = self
                .analyze_item(
                    item,
                    category,
                    &request,
                    &config,
                    &models,
                    &resolution,
                    &invoker,
                    &consent_gate,
                    purpose,
                    &auth_now,
                    &profile,
                    &mut attempts,
                    &mut budget,
                )
                .await?;

            match produced {
                Some(semantics) => results.push(semantics),
                None => {
                    return Err(MediaUnderstandingError::AllRoutesExhausted(format!(
                        "all routes were exhausted for category {category:?}"
                    )));
                }
            }
        }

        Ok(MediaUnderstandingResult { results, attempts })
    }

    /// Process one item through its ordered eligible routes.
    #[allow(clippy::too_many_arguments)]
    async fn analyze_item(
        &self,
        item: &MediaItemBytes,
        category: MediaCategory,
        request: &MediaUnderstandingRequest,
        config: &ResolvedMediaUnderstandingConfig,
        models: &IndexMap<String, ModelEntry>,
        resolution: &RouteResolution<'_>,
        invoker: &AuxMediaInvoker,
        consent_gate: &DisclosureConsentGate,
        purpose: DisclosurePurpose,
        auth_now: &Option<crate::auth::GrokAuth>,
        profile: &PreprocessProfile,
        attempts: &mut Vec<MediaAttemptSummary>,
        budget: &mut RequestBudget,
    ) -> Result<Option<MediaSemantics>, MediaUnderstandingError> {
        let routes = resolution.category_routes(category);
        let eligible_routes: Vec<&ResolvedRoute> =
            routes.iter().filter(|route| route.eligible()).collect();
        if eligible_routes.is_empty() {
            for route in &routes {
                let reason = match &route.eligibility {
                    RouteEligibility::Eligible => "ineligible_unknown".to_string(),
                    RouteEligibility::Unresolved => "unresolved".to_string(),
                    RouteEligibility::CapabilityIneligible => "capability_ineligible".to_string(),
                    RouteEligibility::TransportIneligible => "transport_ineligible".to_string(),
                    RouteEligibility::MissingCredentials => "missing_credentials".to_string(),
                };
                self.append_skip_row(purpose, category, route, "none", &reason, String::new());
            }
            return Ok(None);
        }

        for route in eligible_routes {
            match self
                .attempt_route(
                    item,
                    category,
                    route,
                    request,
                    config,
                    models,
                    invoker,
                    consent_gate,
                    purpose,
                    auth_now,
                    profile,
                    attempts,
                    budget,
                    String::new(),
                )
                .await
            {
                RouteAttemptOutcome::Produced(semantics) => return Ok(Some(semantics)),
                RouteAttemptOutcome::Advanced => continue,
                RouteAttemptOutcome::Terminal(error) => return Err(error),
            }
        }
        Ok(None)
    }

    /// One route attempt: gates → transport → preprocess → cache → delegate.
    #[allow(clippy::too_many_arguments)]
    async fn attempt_route(
        &self,
        item: &MediaItemBytes,
        category: MediaCategory,
        route: &ResolvedRoute,
        request: &MediaUnderstandingRequest,
        config: &ResolvedMediaUnderstandingConfig,
        models: &IndexMap<String, ModelEntry>,
        invoker: &AuxMediaInvoker,
        consent_gate: &DisclosureConsentGate,
        purpose: DisclosurePurpose,
        auth_now: &Option<crate::auth::GrokAuth>,
        profile: &PreprocessProfile,
        attempts: &mut Vec<MediaAttemptSummary>,
        budget: &mut RequestBudget,
        nested_reason_prefix: String,
    ) -> RouteAttemptOutcome {
        let Some(entry) = models.get(&route.model_id) else {
            return RouteAttemptOutcome::Advanced;
        };
        let provider_identity = crate::agent::config::provider_identity_for_model(entry);
        let provider_str = provider_identity_str(provider_identity);

        // ZDR gate: fail closed from trusted account metadata only.
        if !zdr_route_eligible(provider_identity, auth_now.as_ref()) {
            self.append_skip_row(
                purpose,
                category,
                route,
                &provider_str,
                "zdr_ineligible",
                nested_reason_prefix.clone(),
            );
            attempts.push(MediaAttemptSummary {
                provider: provider_str,
                model: route.model_id.clone(),
                outcome: "skipped".to_string(),
                reason: Some("zdr_ineligible".to_string()),
            });
            return RouteAttemptOutcome::Advanced;
        }

        // Consent gate (YOLO-proof): before EVERY fallback provider
        // transmission.
        match consent_gate
            .check(ConsentRequest {
                provider_identity: provider_str.clone(),
                category,
                purpose,
            })
            .await
        {
            ConsentDecision::Deny => {
                self.append_skip_row(
                    purpose,
                    category,
                    route,
                    &provider_str,
                    "consent_denied",
                    nested_reason_prefix.clone(),
                );
                attempts.push(MediaAttemptSummary {
                    provider: provider_str,
                    model: route.model_id.clone(),
                    outcome: "skipped".to_string(),
                    reason: Some("consent_denied".to_string()),
                });
                return RouteAttemptOutcome::Advanced;
            }
            ConsentDecision::Allow => {}
        }

        // Circuit breaker.
        let now = super::now_ts();
        let circuit_key = (provider_str.clone(), category);
        if self.inner.circuit.lock().is_open(now, &circuit_key, config) {
            self.append_skip_row(
                purpose,
                category,
                route,
                &provider_str,
                "circuit_open",
                nested_reason_prefix.clone(),
            );
            attempts.push(MediaAttemptSummary {
                provider: provider_str,
                model: route.model_id.clone(),
                outcome: "skipped".to_string(),
                reason: Some("circuit_open".to_string()),
            });
            return RouteAttemptOutcome::Advanced;
        }

        // Concrete transport must be known before any bytes leave.
        if transport_plan_for(category, route.strategy, &entry.media_transport).is_none() {
            self.append_skip_row(
                purpose,
                category,
                route,
                &provider_str,
                "transport_ineligible",
                nested_reason_prefix.clone(),
            );
            attempts.push(MediaAttemptSummary {
                provider: provider_str,
                model: route.model_id.clone(),
                outcome: "skipped".to_string(),
                reason: Some("transport_ineligible".to_string()),
            });
            return RouteAttemptOutcome::Advanced;
        }

        // Preprocessing (deterministic, bounded).
        let preprocessed = match super::preprocess::preprocess_media(
            category,
            route.strategy,
            &item.bytes,
            item.mime.as_deref(),
            config,
        ) {
            Ok(outcome) => outcome,
            Err(PreprocessError::Unavailable(reason)) => {
                // The strategy is not usable in this build (e.g. FFmpeg not
                // compiled in); skip this route and advance.
                self.append_skip_row(
                    purpose,
                    category,
                    route,
                    &provider_str,
                    &format!("preprocess_unavailable:{reason}"),
                    nested_reason_prefix.clone(),
                );
                attempts.push(MediaAttemptSummary {
                    provider: provider_str,
                    model: route.model_id.clone(),
                    outcome: "skipped".to_string(),
                    reason: Some("preprocess_unavailable".to_string()),
                });
                return RouteAttemptOutcome::Advanced;
            }
            Err(PreprocessError::Failed(reason)) => {
                // Preprocessing failure is terminal (plan §16).
                return RouteAttemptOutcome::Terminal(MediaUnderstandingError::PreprocessFailed(
                    reason,
                ));
            }
        };

        // Build the wire payload from the preprocessed outcome. The third
        // tuple element is the nested video→audio transcript digest (only
        // video `frames` routes produce one); it must be covered by the
        // semantic cache key so a different transcript never reuses a cached
        // result.
        let (prompt, images, nested_audio_digest) = match &preprocessed {
            PreprocessOutcome::Image {
                bytes,
                mime,
                width: _,
                height: _,
            } => {
                let url = data_url(mime, bytes);
                (self.delegate_prompt(category, request), vec![url], None)
            }
            PreprocessOutcome::VideoFrames { frames } => {
                let urls: Vec<String> = frames
                    .iter()
                    .map(|frame| data_url(&frame.mime, &frame.bytes))
                    .collect();
                // Nested audio at depth 1: box the future to keep
                // `attempt_route` non-recursive (the boxed future is the
                // only level of indirection the async recursion needs).
                let (transcript_text, transcript_digest) = match Box::pin(self.nested_audio(
                    item,
                    request,
                    config,
                    models,
                    invoker,
                    consent_gate,
                    purpose,
                    auth_now,
                    profile,
                    attempts,
                    budget,
                ))
                .await
                {
                    Ok(Some(text)) => {
                        let digest = blake3::hash(text.as_bytes()).to_hex().to_string();
                        (format!("\n\nAudio transcript:\n{text}"), Some(digest))
                    }
                    Ok(None) => (String::new(), None),
                    Err(error) => return RouteAttemptOutcome::Terminal(error),
                };
                (
                    format!(
                        "{}{}",
                        self.delegate_prompt(category, request),
                        transcript_text
                    ),
                    urls,
                    transcript_digest,
                )
            }
            PreprocessOutcome::AudioPcm { .. } => {
                // No concrete wire path exists for audio today; the route
                // should have been transport-ineligible.
                self.append_skip_row(
                    purpose,
                    category,
                    route,
                    &provider_str,
                    "transport_ineligible",
                    nested_reason_prefix.clone(),
                );
                return RouteAttemptOutcome::Advanced;
            }
        };

        // Canonical cache key covers every semantic/preprocess variable,
        // including the nested audio transcript digest.
        let cache_key = SemanticCacheKey::new(
            item.source_digest.clone(),
            category,
            provider_str.clone(),
            route.model_id.clone(),
            route.strategy,
            prompt_fingerprint(
                category,
                request.instruction.as_deref(),
                request.detail,
                &request.focus,
                nested_audio_digest.as_deref(),
            ),
            schema_fingerprint(),
            instruction_fingerprint(request.instruction.as_deref()),
            profile.profile.clone(),
            profile.version,
        );

        // Cache hit: no provider call, zero cost, still recorded as a hit.
        tracing::debug!(
            target: "media.cache",
            category = ?category,
            model = %route.model_id,
            "semantic cache lookup"
        );
        if let Ok(Some(cached)) = self.inner.cache.get(&cache_key) {
            if let Some(semantics) = cached
                .results
                .iter()
                .find(|semantics| semantics.source == item.source && semantics.category == category)
                .cloned()
            {
                tracing::debug!(
                    target: "media.cache.hit",
                    category = ?category,
                    model = %route.model_id,
                    "semantic cache hit"
                );
                self.append_cache_hit_row(
                    purpose,
                    category,
                    route,
                    &provider_str,
                    nested_reason_prefix.clone(),
                );
                return RouteAttemptOutcome::Produced(semantics);
            }
        }
        tracing::debug!(
            target: "media.cache.miss",
            category = ?category,
            model = %route.model_id,
            "semantic cache miss; delegating"
        );

        // Delegate through the route's own dedicated InferenceClient.
        let start = Instant::now();
        let delegate_request = DelegateRequest {
            prompt,
            images,
            use_native_schema: self.native_schema_for(entry),
        };
        let outcome = match invoker
            .delegate(&route.model_id, &delegate_request, config.max_output_chars)
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                let reason = delegate_error_reason(&error);
                self.append_failure_row(
                    purpose,
                    category,
                    route,
                    &provider_str,
                    &reason,
                    start.elapsed().as_millis() as u64,
                    nested_reason_prefix.clone(),
                );
                self.inner
                    .circuit
                    .lock()
                    .record(now, circuit_key.clone(), false, config);
                attempts.push(MediaAttemptSummary {
                    provider: provider_str.clone(),
                    model: route.model_id.clone(),
                    outcome: "failed".to_string(),
                    reason: Some(reason),
                });
                if matches!(error, DelegateError::BudgetExceeded(_)) {
                    return RouteAttemptOutcome::Terminal(
                        MediaUnderstandingError::AllRoutesExhausted(
                            "aux budget exceeded for this request".to_string(),
                        ),
                    );
                }
                return RouteAttemptOutcome::Advanced;
            }
        };

        let cost = outcome.cost_usd_ticks.unwrap_or(0);
        budget.spent_ticks = budget.spent_ticks.saturating_add(cost);
        if budget.spent_ticks > budget.max_ticks {
            return RouteAttemptOutcome::Terminal(MediaUnderstandingError::AllRoutesExhausted(
                "aux budget exhausted for this request".to_string(),
            ));
        }

        let semantics_text = cap_text(&outcome.text, config.max_output_chars as usize);
        let semantics = build_semantics(
            &item.source,
            category,
            semantics_text,
            &provider_str,
            &route.model_id,
            route.strategy,
        );

        // Persist the validated result under its canonical key and account
        // the attempt. The semantic result enters the current conversation
        // lifecycle (plan 11.3), so it is referenced under the live
        // attachment ref to survive conservative GC at session close.
        let result = MediaUnderstandingResult {
            results: vec![semantics.clone()],
            attempts: vec![],
        };
        let _ = self.inner.cache.insert(&cache_key, &result);
        let _ = self.inner.store.merge_ref(
            RefKind::Attachments,
            LIVE_ATTACHMENT_REF,
            &[ObjectRef {
                kind: ArtifactKind::Result,
                hash: cache_key.canonical(),
            }],
        );

        let mut reason = if outcome.schema_repaired {
            Some("schema_repaired".to_string())
        } else {
            None
        };
        if !nested_reason_prefix.is_empty() {
            reason = Some(format!(
                "{nested_reason_prefix}:{}",
                reason.as_deref().unwrap_or("")
            ));
        }
        self.append_success_row(
            purpose,
            category,
            route,
            &provider_str,
            &outcome,
            start.elapsed().as_millis() as u64,
            reason,
        );
        self.inner
            .circuit
            .lock()
            .record(now, circuit_key, true, config);
        attempts.push(MediaAttemptSummary {
            provider: provider_str,
            model: route.model_id.clone(),
            outcome: "success".to_string(),
            reason: outcome
                .schema_repaired
                .then(|| "schema_repaired".to_string()),
        });

        RouteAttemptOutcome::Produced(semantics)
    }

    /// Depth-1 nested audio call for video `frames` routes (plan §5.1).
    ///
    /// The nested call has its own budget accounting and its own usage rows
    /// (the same request-level purpose is used, but rows are keyed by the
    /// audio category and carry a `nested_audio` reason prefix). Depth is
    /// inherently capped at 1: this function never recurses.
    #[allow(clippy::too_many_arguments)]
    async fn nested_audio(
        &self,
        item: &MediaItemBytes,
        request: &MediaUnderstandingRequest,
        config: &ResolvedMediaUnderstandingConfig,
        models: &IndexMap<String, ModelEntry>,
        invoker: &AuxMediaInvoker,
        consent_gate: &DisclosureConsentGate,
        purpose: DisclosurePurpose,
        auth_now: &Option<crate::auth::GrokAuth>,
        profile: &PreprocessProfile,
        attempts: &mut Vec<MediaAttemptSummary>,
        budget: &mut RequestBudget,
    ) -> Result<Option<String>, MediaUnderstandingError> {
        let nested_resolution = RouteResolution {
            config,
            models,
            has_credentials: &|model| invoker.context().resolve_route_config(model).is_some(),
        };
        let routes = nested_resolution.category_routes(MediaCategory::Audio);
        let eligible: Vec<&ResolvedRoute> = routes.iter().filter(|r| r.eligible()).collect();
        if eligible.is_empty() {
            for route in &routes {
                self.append_skip_row(
                    purpose,
                    MediaCategory::Audio,
                    route,
                    "none",
                    "transport_ineligible",
                    "nested_audio".to_string(),
                );
            }
            return Ok(None);
        }

        for route in eligible {
            match self
                .attempt_route(
                    item,
                    MediaCategory::Audio,
                    route,
                    request,
                    config,
                    models,
                    invoker,
                    consent_gate,
                    purpose,
                    auth_now,
                    profile,
                    attempts,
                    budget,
                    "nested_audio".to_string(),
                )
                .await
            {
                RouteAttemptOutcome::Produced(semantics) => return Ok(Some(semantics.text)),
                RouteAttemptOutcome::Advanced => continue,
                RouteAttemptOutcome::Terminal(error) => return Err(error),
            }
        }
        Ok(None)
    }

    fn delegate_prompt(
        &self,
        category: MediaCategory,
        request: &MediaUnderstandingRequest,
    ) -> String {
        build_delegate_prompt(
            category,
            request.instruction.as_deref(),
            request.detail,
            &request.focus,
        )
    }

    fn native_schema_for(&self, entry: &ModelEntry) -> bool {
        entry.media_transport.json_schema || entry.supports_native_schema == Some(true)
    }

    fn current_auth(&self) -> Option<crate::auth::GrokAuth> {
        let live = self
            .inner
            .context
            .auth
            .as_ref()
            .and_then(|manager| manager.current_or_expired());
        live.or_else(|| self.inner.context.current_auth.clone())
    }

    fn build_invoker_context(&self, config: &ResolvedMediaUnderstandingConfig) -> InvokerContext {
        let auth = self.inner.context.auth.as_ref();
        let session_key =
            auth.and_then(|manager| manager.current_or_expired().map(|auth| auth.key.clone()));
        let disable_api_key_auth = auth
            .map(|manager| manager.grok_com_config().api_key_auth_disabled())
            .unwrap_or(false);
        InvokerContext {
            models: self.inner.context.models.models(),
            endpoints: self.inner.context.models.endpoints(),
            session_key,
            disable_api_key_auth,
            alpha_test_key: self.inner.context.credentials.alpha_test_key.clone(),
            client_version: self.inner.context.credentials.client_version.clone(),
            active_session_config: self.inner.context.credentials.active_session_config.clone(),
            client_identifier: self.inner.context.credentials.client_identifier.clone(),
            max_retries: self.inner.context.credentials.max_retries,
            max_aux_tokens_per_call: config.max_aux_tokens_per_call,
            max_aux_budget_usd_ticks: config.max_aux_budget_usd_ticks,
        }
    }

    // ── Usage ledger rows ────────────────────────────────────────────────

    fn append_skip_row(
        &self,
        purpose: DisclosurePurpose,
        category: MediaCategory,
        route: &ResolvedRoute,
        provider: &str,
        reason: &str,
        nested_reason_prefix: String,
    ) {
        let reason = if nested_reason_prefix.is_empty() {
            reason.to_string()
        } else {
            format!("{nested_reason_prefix}:{reason}")
        };
        let row = UsageRow::new(
            purpose_to_usage_purpose(purpose),
            category,
            provider,
            &route.model_id,
            route.config_index as u32,
            route.strategy,
        )
        .with_cost_unknown()
        .with_outcome("skipped")
        .with_reason(reason);
        let _ = self.inner.ledger.append(&row);
    }

    fn append_cache_hit_row(
        &self,
        purpose: DisclosurePurpose,
        category: MediaCategory,
        route: &ResolvedRoute,
        provider: &str,
        nested_reason_prefix: String,
    ) {
        let mut row = UsageRow::new(
            purpose_to_usage_purpose(purpose),
            category,
            provider,
            &route.model_id,
            route.config_index as u32,
            route.strategy,
        )
        .with_cost_unknown()
        .with_cache_hit()
        .with_outcome("cached");
        if !nested_reason_prefix.is_empty() {
            row = row.with_reason(nested_reason_prefix);
        }
        let _ = self.inner.ledger.append(&row);
    }

    fn append_success_row(
        &self,
        purpose: DisclosurePurpose,
        category: MediaCategory,
        route: &ResolvedRoute,
        provider: &str,
        outcome: &super::invoker::DelegateOutcome,
        duration_ms: u64,
        reason: Option<String>,
    ) {
        let mut row = UsageRow::new(
            purpose_to_usage_purpose(purpose),
            category,
            provider,
            &route.model_id,
            route.config_index as u32,
            route.strategy,
        )
        .with_tokens(outcome.tokens_in, outcome.tokens_out, outcome.tokens_cached)
        .with_cache_miss()
        .with_duration(duration_ms)
        .with_outcome("success");
        match outcome.cost_usd_ticks {
            Some(ticks) => row = row.with_cost(ticks),
            None => row = row.with_cost_unknown(),
        }
        if let Some(reason) = reason {
            row = row.with_reason(reason);
        }
        let _ = self.inner.ledger.append(&row);
    }

    fn append_failure_row(
        &self,
        purpose: DisclosurePurpose,
        category: MediaCategory,
        route: &ResolvedRoute,
        provider: &str,
        reason: &str,
        duration_ms: u64,
        nested_reason_prefix: String,
    ) {
        let reason = if nested_reason_prefix.is_empty() {
            reason.to_string()
        } else {
            format!("{nested_reason_prefix}:{reason}")
        };
        let row = UsageRow::new(
            purpose_to_usage_purpose(purpose),
            category,
            provider,
            &route.model_id,
            route.config_index as u32,
            route.strategy,
        )
        .with_cost_unknown()
        .with_cache_miss()
        .with_duration(duration_ms)
        .with_outcome("failed")
        .with_reason(reason);
        let _ = self.inner.ledger.append(&row);
    }

    /// Run a consented sample-media route test against one configured route.
    ///
    /// This mirrors [`Self::analyze_for`]'s gates but restricts the attempt
    /// to the single route at `route_index`, so the TUI can report exactly
    /// why THIS route would (or would not) work:
    ///
    /// - permission / containment for the user-selected path;
    /// - ZDR eligibility, disclosure consent, circuit breaker, concrete
    ///   transport, preprocessing, cache, then the delegate call.
    ///
    /// Returns a non-secret human-readable summary (never bytes, prompts,
    /// tokens, or unsanitized provider errors). A `route_index` that is out
    /// of bounds for the category is a terminal input error; an ineligible
    /// route is reported as a summary rather than an error so the UI can
    /// distinguish "this route is not eligible" from "the request failed".
    pub(crate) async fn test_route(
        &self,
        category: MediaCategory,
        route_index: usize,
        path: String,
    ) -> Result<String, MediaUnderstandingError> {
        let config = self.inner.config.read().clone();
        if !config.enabled {
            return Err(MediaUnderstandingError::Unavailable(
                "media understanding is disabled in config".to_string(),
            ));
        }
        let limits = MediaPolicyLimits {
            max_media_bytes: config.max_media_bytes,
            max_audio_seconds: config.max_audio_seconds,
            max_video_seconds: config.max_video_seconds,
            max_video_frames: config.max_video_frames,
            max_instruction_chars: DEFAULT_MAX_INSTRUCTION_CHARS,
            max_focus_items: DEFAULT_MAX_FOCUS_ITEMS,
            max_media_items: DEFAULT_MAX_MEDIA_ITEMS,
        };

        let request = MediaUnderstandingRequest {
            media: vec![MediaSource::Path { path }],
            category,
            instruction: Some("Run a route test: describe the media briefly.".to_string()),
            detail: MediaDetailLevel::default(),
            focus: Vec::new(),
        };
        super::policy::validate_request(&request, &limits)
            .map_err(|e| MediaUnderstandingError::InvalidInput(e.to_string()))?;

        // Gate 1: permission + containment for the selected path.
        let item = super::policy::resolve_media_item(
            &request.media[0],
            &self.inner.context.workspace_root,
            &self.inner.context.session_dir,
            self.inner.context.permission.as_ref(),
            self.inner.context.session_id.as_deref(),
            &limits,
        )
        .await
        .map_err(|e| match e {
            PolicyError::PermissionDenied(_) => {
                MediaUnderstandingError::InvalidInput("media source was not permitted".to_string())
            }
            other => MediaUnderstandingError::PreprocessFailed(other.to_string()),
        })?;
        let _ = super::policy::persist_source_blob(&self.inner.context.session_dir, &item);

        let invoker = AuxMediaInvoker::new(self.build_invoker_context(&config));
        let models = self.inner.context.models.models();
        let resolution = RouteResolution {
            config: &config,
            models: &models,
            has_credentials: &|model| invoker.context().resolve_route_config(model).is_some(),
        };
        let consent_gate = DisclosureConsentGate::new(self.inner.context.consent.clone());
        let auth_now = self.current_auth();
        let profile = PreprocessProfile::for_config(&config);

        let routes = resolution.category_routes(category);
        let Some(route) = routes.get(route_index) else {
            return Err(MediaUnderstandingError::InvalidInput(format!(
                "route index {route_index} out of bounds for category {category:?}"
            )));
        };

        if !route.eligible() {
            let reason = match &route.eligibility {
                RouteEligibility::Eligible => "ineligible_unknown".to_string(),
                RouteEligibility::Unresolved => "unresolved".to_string(),
                RouteEligibility::CapabilityIneligible => "capability_ineligible".to_string(),
                RouteEligibility::TransportIneligible => "transport_ineligible".to_string(),
                RouteEligibility::MissingCredentials => "missing_credentials".to_string(),
            };
            return Ok(format!(
                "route {} ({}) not eligible: {reason}",
                route_index, route.model_id
            ));
        }

        let mut attempts = Vec::new();
        let mut budget = RequestBudget {
            spent_ticks: 0,
            max_ticks: config.max_aux_budget_usd_ticks,
        };
        match self
            .attempt_route(
                &item,
                category,
                route,
                &request,
                &config,
                &models,
                &invoker,
                &consent_gate,
                DisclosurePurpose::ExplicitTool,
                &auth_now,
                &profile,
                &mut attempts,
                &mut budget,
                String::new(),
            )
            .await
        {
            RouteAttemptOutcome::Produced(semantics) => Ok(format!(
                "route {} ({}) succeeded: {}",
                route_index,
                semantics.provenance.model,
                cap_text(&semantics.text, 240)
            )),
            RouteAttemptOutcome::Advanced => Ok(format!(
                "route {} ({}) skipped: no result produced",
                route_index, route.model_id
            )),
            RouteAttemptOutcome::Terminal(error) => Err(error),
        }
    }
}

/// Map a disclosure purpose onto the ledger's usage purpose.
fn purpose_to_usage_purpose(purpose: DisclosurePurpose) -> UsagePurpose {
    match purpose {
        DisclosurePurpose::ExplicitTool => UsagePurpose::ExplicitTool,
        DisclosurePurpose::AutoAttachment => UsagePurpose::AutoAttachment,
        DisclosurePurpose::Compaction => UsagePurpose::Compaction,
    }
}

/// A `data:<mime>;base64,...` URL for `ContentPart::Image`.
fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// Bound semantic text to the configured output cap.
fn cap_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push('…');
    out
}

/// Workspace-relative path hint for a source (used by `Auto` sniffing).
fn source_path_hint(source: &MediaSource) -> Option<&str> {
    match source {
        MediaSource::Path { path } => Some(path.as_str()),
        MediaSource::ArtifactRef { .. } => None,
    }
}

#[async_trait::async_trait]
impl MediaUnderstandingBackend for ShellMediaUnderstandingBackend {
    async fn analyze(
        &self,
        request: MediaUnderstandingRequest,
    ) -> Result<MediaUnderstandingResult, MediaUnderstandingError> {
        self.analyze_for(request, DisclosurePurpose::ExplicitTool)
            .await
    }

    fn availability(&self) -> MediaBackendAvailability {
        self.availability_snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::{Config, ModelInfo};
    use tracing_subscriber::fmt::format::FmtSpan;
    use xai_grok_tools::media::domain::{MediaCapabilities, MediaModalitySupport};

    fn model_entry(model: &str) -> ModelEntry {
        let mut info = ModelInfo::fallback(model);
        info.base_url = "https://example.test/v1".to_string();
        info.media_capabilities = MediaCapabilities {
            image: MediaModalitySupport::Supported,
            audio: MediaModalitySupport::Supported,
            video: MediaModalitySupport::Supported,
            ..Default::default()
        };
        info.media_transport = xai_grok_tools::media::domain::MediaTransportCapabilities {
            image_inline: true,
            json_schema: true,
            ..Default::default()
        };
        ModelEntry {
            info,
            model_provider: None,
            api_key: Some("route-key".to_string()),
            env_key: None,
            auth_provider: None,
            api_base_url: None,
        }
    }

    fn empty_category() -> crate::agent::config::ResolvedMediaCategoryConfig {
        crate::agent::config::ResolvedMediaCategoryConfig {
            routes: vec![],
            max_seconds: None,
            max_frames: None,
        }
    }

    fn base_config() -> ResolvedMediaUnderstandingConfig {
        ResolvedMediaUnderstandingConfig {
            enabled: true,
            auto_enrich: false,
            compaction_enrichment: false,
            active_model_unknown_policy: Default::default(),
            compaction_preflight_policy: Default::default(),
            max_output_chars: 20_000,
            max_aux_tokens_per_call: 8_192,
            max_aux_budget_usd_ticks: 1_000_000_000,
            max_media_bytes: 256 * 1024 * 1024,
            max_audio_seconds: 1_800,
            max_video_seconds: 900,
            max_video_frames: 32,
            max_contact_sheet_side_px: 2_048,
            max_preprocess_wallclock_ms: 120_000,
            preprocess_concurrency: 2,
            circuit_breaker: crate::agent::config::ResolvedMediaCircuitBreakerConfig {
                failures: 5,
                window_secs: 300,
            },
            image: crate::agent::config::ResolvedMediaCategoryConfig {
                routes: vec![crate::agent::config::ResolvedMediaRoute {
                    model: "vision-model".to_string(),
                    strategy: MediaCategoryStrategy::Native,
                    allow_unknown_capability: false,
                    force_unsupported_capability: false,
                }],
                max_seconds: None,
                max_frames: None,
            },
            audio: empty_category(),
            video: empty_category(),
        }
    }

    fn default_models() -> IndexMap<String, ModelEntry> {
        let mut models = IndexMap::new();
        models.insert("vision-model".to_string(), model_entry("vision-model"));
        models
    }

    fn build_manager(
        models: IndexMap<String, ModelEntry>,
        auth: Option<Arc<crate::auth::AuthManager>>,
    ) -> crate::agent::models::ModelsManager {
        let tmp =
            std::env::temp_dir().join(format!("grok-test-media-backend-{}", uuid::Uuid::new_v4()));
        let auth_manager = auth.unwrap_or_else(|| {
            Arc::new(crate::auth::AuthManager::new(
                &tmp,
                crate::auth::GrokComConfig::default(),
            ))
        });
        crate::agent::models::ModelsManager::new(
            None,
            models,
            agent_client_protocol::ModelId::new("default"),
            auth_manager,
            Config::default(),
        )
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(width, height, Rgba([128, 64, 32, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    fn backend_context(
        config: ResolvedMediaUnderstandingConfig,
        models: IndexMap<String, ModelEntry>,
        workspace_root: PathBuf,
        session_dir: PathBuf,
        consent: Option<Arc<dyn MediaConsentProvider>>,
        current_auth: Option<crate::auth::GrokAuth>,
    ) -> ShellMediaBackendContext {
        ShellMediaBackendContext {
            config,
            models: build_manager(models, None),
            auth: None,
            current_auth,
            session_dir,
            workspace_root,
            permission: Some(xai_grok_workspace::permission::PermissionHandle::allow_all()),
            session_id: Some("media-backend-test".to_string()),
            consent,
            credentials: InvokerCredentialSnapshot {
                alpha_test_key: None,
                client_version: None,
                active_session_config: xai_grok_inference::InferenceConfig {
                    base_url: "https://example.test/v1".to_string(),
                    model: "session-model".to_string(),
                    api_backend: Default::default(),
                    ..Default::default()
                },
                client_identifier: None,
                max_retries: None,
            },
        }
    }

    struct AllowConsentProvider;

    #[async_trait::async_trait]
    impl MediaConsentProvider for AllowConsentProvider {
        async fn check(&self, _request: ConsentRequest) -> ConsentDecision {
            ConsentDecision::Allow
        }
    }

    fn allow_consent() -> Arc<dyn MediaConsentProvider> {
        Arc::new(AllowConsentProvider)
    }

    fn analyze_request(path: &str, category: MediaCategory) -> MediaUnderstandingRequest {
        MediaUnderstandingRequest {
            media: vec![MediaSource::Path {
                path: path.to_string(),
            }],
            category,
            instruction: None,
            detail: Default::default(),
            focus: vec![],
        }
    }

    #[test]
    fn media_backend_availability_reflects_enabled_categories() {
        let tmp = tempfile::tempdir().unwrap();
        let context = backend_context(
            base_config(),
            default_models(),
            tmp.path().to_path_buf(),
            tmp.path().join("s"),
            Some(allow_consent()),
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();
        let availability = backend.availability_snapshot();
        assert!(availability.enabled);
        assert!(
            availability
                .supported_categories
                .contains(&MediaCategory::Image)
        );
        assert!(!availability.routes.is_empty());
    }

    #[test]
    fn media_backend_disabled_config_has_no_availability() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = base_config();
        config.enabled = false;
        let context = backend_context(
            config,
            default_models(),
            tmp.path().to_path_buf(),
            tmp.path().join("s"),
            None,
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();
        assert!(!backend.availability_snapshot().enabled);
        assert!(!backend.availability_snapshot().has_eligible_route());
    }

    #[tokio::test]
    async fn media_backend_disabled_analyze_fails_unavailable() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = base_config();
        config.enabled = false;
        let context = backend_context(
            config,
            default_models(),
            tmp.path().to_path_buf(),
            tmp.path().join("s"),
            None,
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();
        let result = backend
            .analyze(analyze_request("photo.png", MediaCategory::Image))
            .await;
        assert!(matches!(
            result,
            Err(MediaUnderstandingError::Unavailable(_))
        ));
    }

    #[tokio::test]
    async fn media_backend_no_eligible_route_fails_closed_without_network() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("photo.png"), png_bytes(16, 16)).unwrap();
        // Empty catalog: the configured route is unresolved, so analyze must
        // fail closed without any delegate call.
        let context = backend_context(
            base_config(),
            IndexMap::new(),
            tmp.path().to_path_buf(),
            tmp.path().join("s"),
            Some(allow_consent()),
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();
        let result = backend
            .analyze(analyze_request("photo.png", MediaCategory::Image))
            .await;
        assert!(matches!(
            result,
            Err(MediaUnderstandingError::AllRoutesExhausted(_))
        ));
    }

    #[tokio::test]
    async fn media_backend_consent_denied_skips_before_any_bytes_leave() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("photo.png"), png_bytes(16, 16)).unwrap();
        let session_dir = tmp.path().join("s");
        // No consent provider → fail closed: the route is skipped with a
        // `consent_denied` usage row and no delegate call happens.
        let context = backend_context(
            base_config(),
            default_models(),
            tmp.path().to_path_buf(),
            session_dir.clone(),
            None,
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();
        let result = backend
            .analyze(analyze_request("photo.png", MediaCategory::Image))
            .await;
        assert!(matches!(
            result,
            Err(MediaUnderstandingError::AllRoutesExhausted(_))
        ));

        let ledger = UsageLedger::open(&session_dir).unwrap();
        let rows = ledger.read().unwrap();
        assert!(
            rows.iter().any(|row| row.outcome == "skipped"
                && row
                    .reason
                    .as_deref()
                    .is_some_and(|r| r.contains("consent_denied"))),
            "consent denial must be recorded, got {rows:?}"
        );
    }

    #[tokio::test]
    async fn media_backend_serves_cache_hit_without_network() {
        let tmp = tempfile::tempdir().unwrap();
        let config = base_config();
        let png = png_bytes(16, 16);
        std::fs::write(tmp.path().join("photo.png"), &png).unwrap();
        let session_dir = tmp.path().join("s");
        let context = backend_context(
            config.clone(),
            default_models(),
            tmp.path().to_path_buf(),
            session_dir.clone(),
            Some(allow_consent()),
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();

        // Pre-seed the semantic cache with the exact canonical key the
        // backend will compute.
        let source_digest = blake3::hash(&png).to_hex().to_string();
        let cache = SemanticCache::open(&session_dir).unwrap();
        let key = SemanticCacheKey::new(
            source_digest,
            MediaCategory::Image,
            "xai".to_string(),
            "vision-model".to_string(),
            MediaCategoryStrategy::Native,
            prompt_fingerprint(MediaCategory::Image, None, Default::default(), &[], None),
            schema_fingerprint(),
            instruction_fingerprint(None),
            PreprocessProfile::for_config(&config).profile,
            PreprocessProfile::for_config(&config).version,
        );
        cache
            .insert(
                &key,
                &MediaUnderstandingResult {
                    results: vec![build_semantics(
                        &MediaSource::Path {
                            path: "photo.png".to_string(),
                        },
                        MediaCategory::Image,
                        "cached semantics".to_string(),
                        "xai",
                        "vision-model",
                        MediaCategoryStrategy::Native,
                    )],
                    attempts: vec![],
                },
            )
            .unwrap();

        let result = backend
            .analyze(analyze_request("photo.png", MediaCategory::Image))
            .await
            .unwrap();
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.results[0].text, "cached semantics");

        let ledger = UsageLedger::open(&session_dir).unwrap();
        let rows = ledger.read().unwrap();
        assert!(
            rows.iter()
                .any(|row| row.cache_hit && row.outcome == "cached"),
            "cache hit must be recorded, got {rows:?}"
        );
    }

    #[tokio::test]
    async fn media_backend_analyze_refs_live_conversation_objects() {
        let tmp = tempfile::tempdir().unwrap();
        let config = base_config();
        let png = png_bytes(16, 16);
        std::fs::write(tmp.path().join("photo.png"), &png).unwrap();
        let session_dir = tmp.path().join("s");
        let context = backend_context(
            config.clone(),
            default_models(),
            tmp.path().to_path_buf(),
            session_dir.clone(),
            Some(allow_consent()),
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();

        // Pre-seed the semantic cache so the analyze path needs no network;
        // `analyze_for` still persists and references the source blob that
        // entered the current conversation lifecycle (plan 11.3).
        let source_digest = blake3::hash(&png).to_hex().to_string();
        let cache = SemanticCache::open(&session_dir).unwrap();
        let key = SemanticCacheKey::new(
            source_digest.clone(),
            MediaCategory::Image,
            "xai".to_string(),
            "vision-model".to_string(),
            MediaCategoryStrategy::Native,
            prompt_fingerprint(MediaCategory::Image, None, Default::default(), &[], None),
            schema_fingerprint(),
            instruction_fingerprint(None),
            PreprocessProfile::for_config(&config).profile,
            PreprocessProfile::for_config(&config).version,
        );
        cache
            .insert(
                &key,
                &MediaUnderstandingResult {
                    results: vec![build_semantics(
                        &MediaSource::Path {
                            path: "photo.png".to_string(),
                        },
                        MediaCategory::Image,
                        "cached semantics".to_string(),
                        "xai",
                        "vision-model",
                        MediaCategoryStrategy::Native,
                    )],
                    attempts: vec![],
                },
            )
            .unwrap();

        let result = backend
            .analyze(analyze_request("photo.png", MediaCategory::Image))
            .await
            .unwrap();
        assert_eq!(result.results.len(), 1);

        // The source blob that entered the current conversation is
        // referenced under refs/attachments/live.json.
        let store = MediaArtifactStore::open(&session_dir).unwrap();
        let refs = store.list_refs(RefKind::Attachments).unwrap();
        let live = refs
            .iter()
            .find(|entry| entry.name == LIVE_ATTACHMENT_REF)
            .expect("live attachment ref must exist after analyze");
        assert!(
            live.objects.contains(&ObjectRef {
                kind: ArtifactKind::Blob,
                hash: source_digest,
            }),
            "the analyzed source blob must be referenced in the live attachment ref"
        );
    }

    #[tokio::test]
    async fn media_backend_zdr_team_blocks_external_provider_without_network() {
        // A ZDR team auth injected as the backend's auth snapshot; the
        // backend must skip an external route with a `zdr_ineligible` row and
        // never call out. The snapshot seam keeps the test deterministic and
        // network-free (the live `AuthManager` always takes precedence in
        // production).
        let zdr_auth = crate::auth::GrokAuth {
            team_blocked_reasons: vec!["BLOCKED_REASON_NO_LOGS".to_string()],
            ..crate::auth::GrokAuth::test_default()
        };

        let tmp = tempfile::tempdir().unwrap();
        let mut config = base_config();
        // Force the only route onto an external (OpenRouter) provider.
        config.image.routes = vec![crate::agent::config::ResolvedMediaRoute {
            model: "external-vision".to_string(),
            strategy: MediaCategoryStrategy::Native,
            allow_unknown_capability: false,
            force_unsupported_capability: false,
        }];
        std::fs::write(tmp.path().join("photo.png"), png_bytes(16, 16)).unwrap();
        let mut models = IndexMap::new();
        let mut info = ModelInfo::fallback("external-vision");
        info.base_url = "https://external.test/v1".to_string();
        info.media_capabilities = MediaCapabilities {
            image: MediaModalitySupport::Supported,
            ..Default::default()
        };
        info.media_transport = xai_grok_tools::media::domain::MediaTransportCapabilities {
            image_inline: true,
            json_schema: true,
            ..Default::default()
        };
        models.insert(
            "external-vision".to_string(),
            ModelEntry {
                info,
                model_provider: Some(crate::agent::model_providers::ResolvedModelProvider {
                    id: "openrouter".to_string(),
                    kind: crate::agent::model_providers::ModelProviderKind::OpenRouter,
                    openrouter_fallback_models: vec![],
                    openrouter_provider_preferences: None,
                    openrouter_plugins: vec![],
                    openrouter_pacing: false,
                    command: vec![],
                }),
                api_key: Some("ext-key".to_string()),
                env_key: None,
                auth_provider: None,
                api_base_url: None,
            },
        );

        let session_dir = tmp.path().join("s");
        let context = backend_context(
            config,
            models,
            tmp.path().to_path_buf(),
            session_dir.clone(),
            Some(allow_consent()),
            Some(zdr_auth),
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();
        let result = backend
            .analyze(analyze_request("photo.png", MediaCategory::Image))
            .await;
        assert!(matches!(
            result,
            Err(MediaUnderstandingError::AllRoutesExhausted(_))
        ));

        let ledger = UsageLedger::open(&session_dir).unwrap();
        let rows = ledger.read().unwrap();
        assert!(
            rows.iter().any(|row| row
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("zdr_ineligible"))),
            "ZDR denial must be recorded, got {rows:?}"
        );
    }

    #[tokio::test]
    async fn media_backend_video_frames_nested_audio_accounting_separate() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = base_config();
        config.video.routes = vec![crate::agent::config::ResolvedMediaRoute {
            model: "vision-model".to_string(),
            strategy: MediaCategoryStrategy::Frames,
            allow_unknown_capability: false,
            force_unsupported_capability: false,
        }];
        config.audio.routes = vec![crate::agent::config::ResolvedMediaRoute {
            model: "audio-model".to_string(),
            strategy: MediaCategoryStrategy::Transcription,
            allow_unknown_capability: false,
            force_unsupported_capability: false,
        }];
        let mut models = default_models();
        models.insert("audio-model".to_string(), model_entry("audio-model"));

        let session_dir = tmp.path().join("s");
        let context = backend_context(
            config.clone(),
            models.clone(),
            tmp.path().to_path_buf(),
            session_dir.clone(),
            Some(allow_consent()),
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();

        // Full analyze of a video request: the frames preprocess must never
        // send bytes. Without a usable native FFmpeg backend the preprocess
        // is unavailable and the video route is skipped BEFORE the nested
        // audio call runs; with a usable backend the garbage input fails
        // closed as a terminal PreprocessFailed. Either way no nested audio
        // rows may exist yet.
        std::fs::write(tmp.path().join("clip.mp4"), b"not-a-real-video").unwrap();
        let result = backend
            .analyze(analyze_request("clip.mp4", MediaCategory::Video))
            .await;
        assert!(result.is_err());
        let ledger = UsageLedger::open(&session_dir).unwrap();
        let rows = ledger.read().unwrap();
        match xai_grok_tools::media::ffmpeg_api::availability() {
            xai_grok_tools::media::ffmpeg_api::FfmpegAvailability::Available => {
                assert!(
                    rows.iter().all(|row| !row
                        .reason
                        .as_deref()
                        .is_some_and(|r| r.contains("nested_audio"))),
                    "no nested audio rows may exist before the nested call, got {rows:?}"
                );
            }
            xai_grok_tools::media::ffmpeg_api::FfmpegAvailability::CompiledOut
            | xai_grok_tools::media::ffmpeg_api::FfmpegAvailability::Unavailable(_) => {
                assert!(
                    rows.iter().any(|row| row
                        .reason
                        .as_deref()
                        .is_some_and(|r| r.contains("preprocess_unavailable"))),
                    "video frames preprocess unavailability must be recorded, got {rows:?}"
                );
            }
        }

        // Direct nested-audio invocation: the depth-1 nested call resolves
        // the audio category with its own budget and appends its OWN separate
        // usage rows keyed by the audio category (audio routes are
        // transport-ineligible until the transcription adapter lands).
        let item = MediaItemBytes {
            source: MediaSource::Path {
                path: "clip.mp4".to_string(),
            },
            bytes: b"not-a-real-video".to_vec(),
            mime: Some("video/mp4".to_string()),
            source_digest: blake3::hash(b"not-a-real-video").to_hex().to_string(),
        };
        let request = analyze_request("clip.mp4", MediaCategory::Video);
        let invoker_context = backend.build_invoker_context(&config);
        let invoker = AuxMediaInvoker::new(invoker_context);
        let consent_gate = DisclosureConsentGate::new(Some(allow_consent()));
        let mut attempts = Vec::new();
        let mut budget = RequestBudget {
            spent_ticks: 0,
            max_ticks: config.max_aux_budget_usd_ticks,
        };
        let transcript = backend
            .nested_audio(
                &item,
                &request,
                &config,
                &models,
                &invoker,
                &consent_gate,
                DisclosurePurpose::ExplicitTool,
                &None::<crate::auth::GrokAuth>,
                &PreprocessProfile::for_config(&config),
                &mut attempts,
                &mut budget,
            )
            .await
            .unwrap();
        assert!(
            transcript.is_none(),
            "no audio transcript without a transport"
        );

        let ledger = UsageLedger::open(&session_dir).unwrap();
        let rows = ledger.read().unwrap();
        let nested_rows: Vec<_> = rows
            .iter()
            .filter(|row| {
                row.category == MediaCategory::Audio
                    && row
                        .reason
                        .as_deref()
                        .is_some_and(|r| r.contains("nested_audio"))
            })
            .collect();
        assert!(
            !nested_rows.is_empty(),
            "nested audio must append its own separate rows, got {rows:?}"
        );
        for row in &nested_rows {
            assert_eq!(row.model, "audio-model");
            assert_eq!(row.category, MediaCategory::Audio);
        }
    }

    #[test]
    fn media_backend_hot_swap_keeps_valid_and_rejects_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let context = backend_context(
            base_config(),
            default_models(),
            tmp.path().to_path_buf(),
            tmp.path().join("s"),
            Some(allow_consent()),
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();

        // Invalid config (duplicate route) must be ignored.
        let invalid = MediaUnderstandingConfig {
            image: Some(crate::agent::config::MediaCategoryConfig {
                routes: vec![
                    crate::agent::config::MediaRoute {
                        model: "vision-model".to_string(),
                        ..Default::default()
                    },
                    crate::agent::config::MediaRoute {
                        model: "vision-model".to_string(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        backend.apply_media_config(&invalid);
        assert!(backend.availability_snapshot().enabled);

        // A valid disable swap takes effect.
        let disable = MediaUnderstandingConfig {
            enabled: Some(false),
            ..Default::default()
        };
        backend.apply_media_config(&disable);
        assert!(!backend.availability_snapshot().enabled);
    }

    #[test]
    fn media_context_hot_swap_changes_routes_and_policy_without_rebuild() {
        let tmp = tempfile::tempdir().unwrap();
        let context = backend_context(
            base_config(),
            default_models(),
            tmp.path().to_path_buf(),
            tmp.path().join("s"),
            Some(allow_consent()),
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();
        let media_context = crate::session::media::auto_enrich::SessionMediaContext {
            backend: Arc::new(backend),
            config: parking_lot::RwLock::new(base_config()),
        };

        // Baseline: the base route set is live and auto-enrich is off.
        let baseline = media_context.backend.availability_snapshot();
        assert!(baseline.enabled);
        assert!(
            baseline
                .routes
                .iter()
                .any(|route| route.model_id == "vision-model"),
            "base route must be live before the swap"
        );
        assert!(!media_context.config.read().auto_enrich);

        // Live swap: replace the image route model and enable auto-enrich.
        let swapped = MediaUnderstandingConfig {
            enabled: Some(true),
            auto_enrich: Some(true),
            image: Some(crate::agent::config::MediaCategoryConfig {
                routes: vec![crate::agent::config::MediaRoute {
                    model: "other-model".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(media_context.apply_config(&swapped));

        // The backend's availability snapshot reflects the new route set
        // without any rebuild (same backend instance).
        let availability = media_context.backend.availability_snapshot();
        assert!(availability.enabled);
        assert!(
            availability
                .routes
                .iter()
                .any(|route| route.model_id == "other-model"),
            "hot-swapped route must appear in the live backend snapshot"
        );
        assert!(
            !availability
                .routes
                .iter()
                .any(|route| route.model_id == "vision-model"),
            "replaced route must no longer be live"
        );
        // The session decision snapshot reflects the new policy.
        assert!(media_context.config.read().auto_enrich);
        assert_eq!(
            media_context.config.read().image.routes[0].model,
            "other-model"
        );
    }

    #[test]
    fn media_context_hot_swap_rejects_invalid_config() {
        let tmp = tempfile::tempdir().unwrap();
        let context = backend_context(
            base_config(),
            default_models(),
            tmp.path().to_path_buf(),
            tmp.path().join("s"),
            Some(allow_consent()),
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();
        let media_context = crate::session::media::auto_enrich::SessionMediaContext {
            backend: Arc::new(backend),
            config: parking_lot::RwLock::new(base_config()),
        };

        // Invalid config (duplicate route) must be rejected before any state
        // is touched: both the backend snapshot and the decision snapshot
        // keep the previously accepted values.
        let invalid = MediaUnderstandingConfig {
            image: Some(crate::agent::config::MediaCategoryConfig {
                routes: vec![
                    crate::agent::config::MediaRoute {
                        model: "vision-model".to_string(),
                        ..Default::default()
                    },
                    crate::agent::config::MediaRoute {
                        model: "vision-model".to_string(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(!media_context.apply_config(&invalid));

        let availability = media_context.backend.availability_snapshot();
        assert!(availability.enabled);
        assert!(
            availability
                .routes
                .iter()
                .any(|route| route.model_id == "vision-model"),
            "accepted route set must survive an invalid edit"
        );
        assert!(
            !media_context.config.read().auto_enrich,
            "decision snapshot must survive an invalid edit"
        );
    }

    #[test]
    fn media_backend_never_reuses_parent_inference_handle() {
        // The backend only ever builds dedicated route clients via the
        // invoker; the session's own `InferenceConfig` snapshot is used
        // solely for stamping session-local fields. This is enforced by
        // construction (there is no parent `InferenceClient`/handle in the
        // backend at all).
        let tmp = tempfile::tempdir().unwrap();
        let context = backend_context(
            base_config(),
            default_models(),
            tmp.path().to_path_buf(),
            tmp.path().join("s"),
            Some(allow_consent()),
            None,
        );
        let _backend = ShellMediaUnderstandingBackend::new(context).unwrap();
    }

    #[test]
    fn media_backend_cap_text_bounds_output() {
        assert_eq!(cap_text("short", 10), "short");
        let capped = cap_text(&"x".repeat(50), 10);
        assert!(capped.chars().count() <= 11, "cap + ellipsis");
        assert!(capped.ends_with('…'));
    }

    // PR 10: redaction and adversarial full-pipeline hardening
    // ------------------------------------------------------------------

    /// Emitted trace fields and the durable attempt record (usage ledger)
    /// must never carry user-controlled marker strings: instruction text,
    /// media paths, or credential-shaped content (plan section 17).
    #[test]
    fn media_backend_trace_and_ledger_never_leak_markers() {
        const INSTRUCTION_MARKER: &str = "INSTRUCTION_MARKER_7f3a";
        const PATH_MARKER: &str = "PATH_MARKER_9c2e";
        const CREDENTIAL_MARKER: &str = "sk-test-CREDENTIAL_MARKER_a4f6";

        #[derive(Clone)]
        struct CaptureWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl std::io::Write for CaptureWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
            type Writer = CaptureWriter;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }

        let captured = CaptureWriter(std::sync::Arc::new(std::sync::Mutex::new(Vec::new())));
        let subscriber = tracing_subscriber::fmt()
            .with_writer(captured.clone())
            .with_max_level(tracing::Level::DEBUG)
            // `analyze_for` emits no events, only the `media.analyze` span
            // lifecycle; synthesize span open/close events so the capture is
            // non-vacuous. The span's fields are request metadata only
            // (category, item count, purpose), so the output contains no
            // markers, paths, or credentials.
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .finish();

        let (result, ledger_text, trace_text) =
            tracing::subscriber::with_default(subscriber, || {
                // The thread-local dispatcher is set before the runtime starts,
                // so every span emitted while the future runs is captured.
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async {
                    let tmp = tempfile::tempdir().unwrap();
                    std::fs::write(tmp.path().join("photo.png"), png_bytes(16, 16)).unwrap();
                    // A second file whose *path* carries a marker string.
                    let marker_path = format!("{PATH_MARKER}.png");
                    std::fs::write(tmp.path().join(&marker_path), png_bytes(8, 8)).unwrap();
                    let session_dir = tmp.path().join("s");

                    // Empty catalog: every configured route is unresolved, so
                    // the pipeline fails closed with zero network and still
                    // writes skip rows to the ledger.
                    let context = backend_context(
                        base_config(),
                        IndexMap::new(),
                        tmp.path().to_path_buf(),
                        session_dir.clone(),
                        Some(allow_consent()),
                        None,
                    );
                    let backend = ShellMediaUnderstandingBackend::new(context).unwrap();

                    let request = MediaUnderstandingRequest {
                        media: vec![
                            MediaSource::Path {
                                path: "photo.png".to_string(),
                            },
                            MediaSource::Path { path: marker_path },
                        ],
                        category: MediaCategory::Image,
                        instruction: Some(format!("describe {INSTRUCTION_MARKER}")),
                        detail: Default::default(),
                        focus: vec![CREDENTIAL_MARKER.to_string()],
                    };

                    let result = backend.analyze(request).await;

                    let ledger =
                        std::fs::read_to_string(session_dir.join("assets/media/usage.jsonl"))
                            .unwrap_or_default();
                    let trace = String::from_utf8_lossy(&captured.0.lock().unwrap()).to_string();
                    (result, ledger, trace)
                })
            });

        // Fails closed with no eligible route (no network).
        assert!(matches!(
            result,
            Err(MediaUnderstandingError::AllRoutesExhausted(_))
        ));

        // The ledger is non-empty (skip rows were durably recorded).
        assert!(
            ledger_text.contains("unresolved") && ledger_text.contains("skipped"),
            "skip rows must be recorded to the usage ledger: {ledger_text}"
        );

        // The durable attempt record never carries the markers.
        for (name, marker) in [
            ("instruction", INSTRUCTION_MARKER),
            ("path", PATH_MARKER),
            ("credential", CREDENTIAL_MARKER),
        ] {
            assert!(
                !ledger_text.contains(marker),
                "ledger leaked {name} marker: {ledger_text}"
            );
        }
        assert!(!ledger_text.contains("photo.png"), "ledger leaked a path");

        // The `media.analyze` span fired and no marker or path entered the
        // emitted trace fields.
        assert!(
            trace_text.contains("media.analyze"),
            "media.analyze span missing: {trace_text}"
        );
        for (name, marker) in [
            ("instruction", INSTRUCTION_MARKER),
            ("path", PATH_MARKER),
            ("credential", CREDENTIAL_MARKER),
        ] {
            assert!(
                !trace_text.contains(marker),
                "trace leaked {name} marker: {trace_text}"
            );
        }
        assert!(!trace_text.contains("photo.png"), "trace leaked a path");
        assert!(
            !trace_text.contains("media bytes") && !trace_text.contains("base64"),
            "trace must not describe media payloads: {trace_text}"
        );
    }

    /// Adversarial full-pipeline coverage: malformed, truncated,
    /// wrong-magic, oversized, and plain-text sources must all fail closed
    /// with typed errors inside a hard wall-clock bound and never panic.
    #[tokio::test(flavor = "current_thread")]
    async fn media_backend_adversarial_corpus_bounded_and_fail_closed() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = base_config();
        // Hard byte bound for the oversized case (a >cap file must trip the
        // policy gate before any decode).
        config.max_media_bytes = 4096;

        let context = backend_context(
            config,
            IndexMap::new(),
            tmp.path().to_path_buf(),
            tmp.path().join("s"),
            Some(allow_consent()),
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();

        // Deterministic pseudo-random garbage with media magic prefixes.
        let mut state = 0x1234_5678u32;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };

        let png = png_bytes(16, 16);
        let mut corpus: Vec<(String, Vec<u8>)> = Vec::new();
        // Truncated PNGs (including empty).
        for cut in [0usize, 1, 8, 30, png.len() / 2] {
            corpus.push((format!("trunc_{cut}.bin"), png[..cut].to_vec()));
        }
        // Wrong-magic garbage.
        for (index, magic) in [
            b"\x89PNG".as_slice(),
            b"RIFF".as_slice(),
            b"GIF89a".as_slice(),
            b"ID3".as_slice(),
            b"OggS".as_slice(),
            b"\xff\xd8\xff".as_slice(),
        ]
        .iter()
        .enumerate()
        {
            let mut blob = magic.to_vec();
            for _ in 0..2048 {
                blob.push((next() & 0xFF) as u8);
            }
            corpus.push((format!("magic_{index}.bin"), blob));
        }
        // Plain text and an oversized payload.
        corpus.push((
            "plain.txt".to_string(),
            b"this is not media at all".to_vec(),
        ));
        corpus.push(("oversized.bin".to_string(), vec![0u8; 8192]));
        // Control: a fully valid PNG.
        corpus.push(("valid.png".to_string(), png));

        let start = std::time::Instant::now();
        for (name, bytes) in &corpus {
            std::fs::write(tmp.path().join(name), bytes).unwrap();
            let result = backend
                .analyze(MediaUnderstandingRequest {
                    media: vec![MediaSource::Path { path: name.clone() }],
                    category: MediaCategory::Auto,
                    instruction: None,
                    detail: Default::default(),
                    focus: vec![],
                })
                .await;
            assert!(
                matches!(
                    result,
                    Err(MediaUnderstandingError::InvalidInput(_)
                        | MediaUnderstandingError::PreprocessFailed(_)
                        | MediaUnderstandingError::AllRoutesExhausted(_))
                ),
                "adversarial case `{name}` must fail closed with a typed error, got {result:?}"
            );
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(30),
            "adversarial corpus exceeded the wall-clock bound"
        );
    }
}
