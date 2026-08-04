//! Compaction preflight media enrichment (plan section 14).
//!
//! PR 8 owns the host-side compaction preflight: before any text-only
//! compaction request is built, every media payload in the stable job
//! snapshot is converted into durable semantics through the shell-owned
//! media-understanding backend.
//!
//! The seam is purpose-scoped:
//!
//! - [`run_compaction_preflight`] is the canonical preflight seam shared by
//!   every compaction caller (full-replace, two-pass, and rolling). It runs
//!   exactly **once per stable job**, and callers derive every input-ladder
//!   stage (verbatim, fitted, lossy), every chunk or bisection range, and
//!   every route fallback from the single [`PreparedCompactionSource`] it
//!   returns, so no live conversation is re-fetched inside the ladder.
//! - [`prepare_media_semantics`] implements the enabled-mode enrichment that
//!   [`run_compaction_preflight`] dispatches to.
//! - Enrichment is delegated through the backend with
//!   [`DisclosurePurpose::Compaction`], which carries its own consent key
//!   (`(provider, category, purpose)`) and is never inherited from the
//!   explicit-tool or auto-attachment grants.
//! - The transform is **pairing-safe**: it replaces `ContentPart::Image`
//!   and `ToolResultItem::images` with scrubbed provenance envelopes while
//!   preserving item order, tool-call/result IDs, error flags, and the
//!   original item count. Live history is never mutated: the preflight only
//!   ever sees the job snapshot copy.
//! - The fingerprint is a BLAKE3 digest of a stable serialization of the
//!   **raw** snapshot, computed before enrichment. Two-pass NOTE₁ staleness
//!   checks keep working because the raw item text/tags are fingerprinted,
//!   not the enriched text.
//! - [`sanitize_compaction_images`] (xai-chat-state) remains the final
//!   defensive net: a correctly enriched conversation has no remaining
//!   `ContentPart::Image` and empty `ToolResultItem::images`, so the
//!   sanitizer is a no-op on it.
//!
//! Policy (plan §14.5):
//!
//! - [`CompactionPreflightPolicy::BestEffort`] (default): use cached/fresh
//!   semantics where available and preserve the existing placeholder path on
//!   failure. It never fails a compaction.
//! - [`CompactionPreflightPolicy::Strict`]: fail the compaction attempt when
//!   required media semantics cannot be produced.
//!
//! The `GROK_DISABLE_MEDIA_COMPACTION_ENRICH` kill switch and the resolved
//! `compaction_enrichment` config flag are honored by the caller
//! ([`super::compaction_enrichment_mode`]); when disabled, the raw snapshot
//! flows through unchanged and the sanitizer keeps the current placeholder
//! behavior exactly.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use xai_grok_inference_types::{ContentPart, ConversationItem, ToolResultItem, UserItem};
use xai_grok_tools::media::backend::{
    MediaSemantics, MediaUnderstandingError, MediaUnderstandingRequest, MediaUnderstandingResult,
};
use xai_grok_tools::media::domain::{MediaCategory, MediaDetailLevel, MediaSource};

use crate::agent::config::CompactionPreflightPolicy;

use super::artifacts::{ArtifactKind, MediaArtifactStore, ObjectRef, RefKind};
use super::auto_enrich::render_semantics_envelope;
use super::backend::ShellMediaUnderstandingBackend;
use super::consent::DisclosurePurpose;

/// Default preflight instruction sent to the delegate route.
///
/// Media-derived text is model-generated and therefore untrusted; it is
/// labeled as such by the provenance envelope. The instruction itself is a
/// fixed, host-owned string — never a user instruction.
const COMPACTION_ENRICH_INSTRUCTION: &str = "Describe the semantic content of each attached image precisely and \
     completely for a successor text-only assistant that must summarize this \
     conversation. Include visible text, UI elements, error messages, \
     diagrams, code, and any information a future assistant needs. Do not \
     invent content that is not visible in the image.";

/// Request batch cap, mirroring the backend's `max_media_items` bound so a
/// single item with many images never trips request validation.
const MAX_IMAGES_PER_REQUEST: usize = 32;

/// Outcome of the compaction-enrichment decision for one job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompactionEnrichmentMode {
    /// Enrichment disabled (kill switch, config flag, or unavailable
    /// backend): the raw snapshot is used and
    /// [`xai_chat_state::compaction_utils::sanitize_compaction_images`]
    /// keeps the current placeholder behavior.
    Disabled,
    /// Enrichment enabled with the configured failure policy.
    Enabled { policy: CompactionPreflightPolicy },
}

/// Whether `GROK_DISABLE_MEDIA_COMPACTION_ENRICH` is set. When set,
/// compaction keeps the current placeholder behavior and no media bytes are
/// delegated (plan §5.5).
pub(crate) fn compaction_enrich_kill_switched() -> bool {
    std::env::var_os("GROK_DISABLE_MEDIA_COMPACTION_ENRICH").is_some()
}

/// Decide the enrichment mode for one compaction job from the resolved media
/// config and the kill switch. Pure so it is unit-testable without a session.
pub(crate) fn compaction_enrichment_mode(
    config: Option<&crate::agent::config::ResolvedMediaUnderstandingConfig>,
    kill_switched: bool,
) -> CompactionEnrichmentMode {
    match config {
        Some(config) if !kill_switched && config.enabled && config.compaction_enrichment => {
            CompactionEnrichmentMode::Enabled {
                policy: config.compaction_preflight_policy,
            }
        }
        _ => CompactionEnrichmentMode::Disabled,
    }
}

/// Stable BLAKE3 fingerprint of the raw compaction snapshot.
///
/// Computed on the **raw** items (pre-enrichment), so a changed snapshot
/// yields a different fingerprint while enrichment itself never alters it.
/// This is the canonical preflight source identity used for stale-snapshot
/// detection (two-pass NOTE₁ cache staleness keeps fingerprinting raw item
/// text/tags).
pub(crate) fn fingerprint_snapshot(snapshot: &[ConversationItem]) -> String {
    let bytes = serde_json::to_vec(snapshot).unwrap_or_default();
    blake3::hash(&bytes).to_hex().to_string()
}

/// Run one canonical compaction preflight (plan §14).
///
/// Shared by every compaction caller — full-replace, two-pass, and rolling —
/// so all of them honor the kill switch, the resolved config, and the
/// strict/best-effort policy identically. Callers derive every input-ladder
/// stage and every route fallback from the single returned
/// [`PreparedCompactionSource`]; the live conversation is never re-fetched
/// inside the ladder.
///
/// - [`CompactionEnrichmentMode::Disabled`] returns the raw snapshot
///   unchanged (placeholder path; the final defensive sanitizer keeps its
///   current behavior).
/// - [`CompactionEnrichmentMode::Enabled`] runs
///   [`prepare_media_semantics`] once with the configured failure policy:
///   best-effort never fails (failed images keep the placeholder path),
///   strict fails the job when required semantics cannot be produced.
///
/// The caller (a `SessionActor`) resolves the mode from its media context
/// via [`compaction_enrichment_mode`]; this function is deliberately pure
/// with respect to the mode so every caller and the unit tests share the
/// exact same seam.
pub(crate) async fn run_compaction_preflight<B: CompactionAnalyzer + ?Sized>(
    backend: &B,
    session_dir: &Path,
    raw: &[ConversationItem],
    mode: CompactionEnrichmentMode,
) -> Result<PreparedCompactionSource, MediaPreflightError> {
    match mode {
        CompactionEnrichmentMode::Disabled => Ok(PreparedCompactionSource {
            snapshot_fingerprint: fingerprint_snapshot(raw),
            enriched: raw.to_vec(),
        }),
        CompactionEnrichmentMode::Enabled { policy } => {
            prepare_media_semantics(backend, session_dir, raw.to_vec(), policy).await
        }
    }
}

/// Result of one compaction preflight.
#[derive(Debug, Clone)]
pub(crate) struct PreparedCompactionSource {
    /// BLAKE3 fingerprint of the raw snapshot this preflight was built from.
    pub(crate) snapshot_fingerprint: String,
    /// The pairing-safe enriched snapshot shared by every input-ladder stage
    /// and route fallback.
    pub(crate) enriched: Vec<ConversationItem>,
}

/// Terminal preflight error. [`CompactionPreflightPolicy::BestEffort`]
/// callers turn every error into the raw snapshot (placeholder path);
/// [`CompactionPreflightPolicy::Strict`] callers fail the job.
#[derive(Debug, Clone, thiserror::Error)]
pub(crate) enum MediaPreflightError {
    #[error("compaction media preflight failed: {0}")]
    Backend(String),
    #[error("compaction media preflight failed to store media bytes: {0}")]
    Store(String),
    #[error("compaction media preflight could not produce required semantics: {0}")]
    Enrichment(String),
}

/// Purpose-scoped analyze seam used by the preflight.
///
/// The public [`xai_grok_tools::media::backend::MediaUnderstandingBackend`]
/// trait only exposes `analyze` (explicit-tool purpose). Compaction needs the
/// `DisclosurePurpose::Compaction` key for the consent gate and the usage
/// ledger, so the preflight delegates through this concrete-purpose seam.
/// Tests implement it with a counting stub.
#[async_trait]
pub(crate) trait CompactionAnalyzer: Send + Sync {
    async fn analyze_for_compaction(
        &self,
        request: MediaUnderstandingRequest,
    ) -> Result<MediaUnderstandingResult, MediaUnderstandingError>;
}

#[async_trait]
impl CompactionAnalyzer for ShellMediaUnderstandingBackend {
    async fn analyze_for_compaction(
        &self,
        request: MediaUnderstandingRequest,
    ) -> Result<MediaUnderstandingResult, MediaUnderstandingError> {
        self.analyze_for(request, DisclosurePurpose::Compaction)
            .await
    }
}

/// One image site inside an item: where it lives and its content address.
struct ImageSite {
    part_index: usize,
    digest: String,
}

/// Run one compaction preflight: fingerprint the raw snapshot, then replace
/// every media payload with a scrubbed provenance envelope.
///
/// Enrichment is delegated through the backend with
/// [`DisclosurePurpose::Compaction`] (via [`CompactionAnalyzer`]).
///
/// - `best_effort` never fails: failed or undecodable images keep their
///   original parts, and the downstream sanitizer preserves the current
///   placeholder behavior.
/// - `strict` fails when required media semantics cannot be produced.
///
/// Traced as `media.compaction.preflight` (plan section 17) with the item
/// count, policy, and a non-reversible BLAKE3 snapshot fingerprint — never
/// media bytes, paths, or semantics text.
#[tracing::instrument(
    name = "media.compaction.preflight",
    skip_all,
    fields(
        items = snapshot.len(),
        policy = ?policy,
        snapshot_fingerprint = tracing::field::Empty,
    )
)]
pub(crate) async fn prepare_media_semantics<B: CompactionAnalyzer + ?Sized>(
    backend: &B,
    session_dir: &Path,
    snapshot: Vec<ConversationItem>,
    policy: CompactionPreflightPolicy,
) -> Result<PreparedCompactionSource, MediaPreflightError> {
    let snapshot_fingerprint = fingerprint_snapshot(&snapshot);
    tracing::Span::current().record("snapshot_fingerprint", &snapshot_fingerprint.as_str());
    if !conversation_has_images(&snapshot) {
        return Ok(PreparedCompactionSource {
            snapshot_fingerprint,
            enriched: snapshot,
        });
    }
    let store = match MediaArtifactStore::open(session_dir) {
        Ok(store) => store,
        Err(error) => {
            if policy == CompactionPreflightPolicy::Strict {
                return Err(MediaPreflightError::Store(error.to_string()));
            }
            // best_effort: keep the raw snapshot; the sanitizer placeholders it.
            return Ok(PreparedCompactionSource {
                snapshot_fingerprint,
                enriched: snapshot,
            });
        }
    };
    let mut digest_to_semantics: HashMap<String, MediaSemantics> = HashMap::new();
    let mut retained: Vec<ObjectRef> = Vec::new();
    let enriched = enrich_items(
        backend,
        &store,
        snapshot,
        &mut digest_to_semantics,
        &mut retained,
        policy,
    )
    .await?;

    // The source artifacts that entered the compaction lifecycle (plan
    // 11.3) are referenced under refs/compaction/<fingerprint>.json so
    // conservative GC at session close retains them and replay of the
    // compacted history can resolve artifact refs. Best-effort: a ref
    // bookkeeping failure never fails the preflight.
    if let Err(error) = store.merge_ref(
        RefKind::Compaction,
        &format!("fp-{snapshot_fingerprint}"),
        &retained,
    ) {
        tracing::warn!(
            %error,
            snapshot_fingerprint = %snapshot_fingerprint,
            "failed to reference compaction media artifacts",
        );
    }

    Ok(PreparedCompactionSource {
        snapshot_fingerprint,
        enriched,
    })
}

/// Whether any conversation item carries a media payload.
fn conversation_has_images(snapshot: &[ConversationItem]) -> bool {
    snapshot.iter().any(|item| match item {
        ConversationItem::User(user) => user
            .content
            .iter()
            .any(|part| matches!(part, ContentPart::Image { .. })),
        ConversationItem::ToolResult(result) => !result.images.is_empty(),
        _ => false,
    })
}

/// Pairing-safe per-item transform over a job snapshot. Item order and count
/// are preserved; only media payloads change. Objects written while enriching
/// are appended to `retained` so the caller can reference them under the
/// compaction namespace.
async fn enrich_items<B: CompactionAnalyzer + ?Sized>(
    backend: &B,
    store: &MediaArtifactStore,
    snapshot: Vec<ConversationItem>,
    digest_to_semantics: &mut HashMap<String, MediaSemantics>,
    retained: &mut Vec<ObjectRef>,
    policy: CompactionPreflightPolicy,
) -> Result<Vec<ConversationItem>, MediaPreflightError> {
    let mut enriched = Vec::with_capacity(snapshot.len());
    for item in snapshot {
        let next = match item {
            ConversationItem::User(user)
                if user
                    .content
                    .iter()
                    .any(|part| matches!(part, ContentPart::Image { .. })) =>
            {
                ConversationItem::User(
                    enrich_user_item(backend, store, user, digest_to_semantics, retained, policy)
                        .await?,
                )
            }
            ConversationItem::ToolResult(result) if !result.images.is_empty() => {
                ConversationItem::ToolResult(
                    enrich_tool_result(
                        backend,
                        store,
                        result,
                        digest_to_semantics,
                        retained,
                        policy,
                    )
                    .await?,
                )
            }
            other => other,
        };
        enriched.push(next);
    }
    Ok(enriched)
}

/// Replace a user message's image parts with scrubbed provenance envelopes.
async fn enrich_user_item<B: CompactionAnalyzer + ?Sized>(
    backend: &B,
    store: &MediaArtifactStore,
    user: UserItem,
    digest_to_semantics: &mut HashMap<String, MediaSemantics>,
    retained: &mut Vec<ObjectRef>,
    policy: CompactionPreflightPolicy,
) -> Result<UserItem, MediaPreflightError> {
    let image_part_count = user
        .content
        .iter()
        .filter(|part| matches!(part, ContentPart::Image { .. }))
        .count();
    if image_part_count == 0 {
        return Ok(user);
    }
    let mut sites = collect_sites(&user.content, store, retained, policy)?;
    if sites.is_empty() {
        if policy == CompactionPreflightPolicy::Strict {
            return Err(MediaPreflightError::Enrichment(
                "user image could not be decoded from the conversation snapshot".to_string(),
            ));
        }
        return Ok(user);
    }
    let analysis = analyze_sites(backend, sites, digest_to_semantics, policy).await?;
    if policy == CompactionPreflightPolicy::Strict
        && (analysis.len() != image_part_count || analysis.iter().any(|(_, s)| s.is_none()))
    {
        return Err(MediaPreflightError::Enrichment(
            "user image semantics could not be produced".to_string(),
        ));
    }
    let mut user = user;
    for (part_index, semantics) in analysis {
        if let Some(semantics) = semantics {
            user.content[part_index] = ContentPart::Text {
                text: Arc::<str>::from(render_semantics_envelope(&semantics)),
            };
        }
    }
    Ok(user)
}

/// Enrich a tool result's inline images. The final defensive sanitizer
/// clears `ToolResultItem::images` wholesale, so the semantic envelope rides
/// in the textual content to survive it.
async fn enrich_tool_result<B: CompactionAnalyzer + ?Sized>(
    backend: &B,
    store: &MediaArtifactStore,
    result: ToolResultItem,
    digest_to_semantics: &mut HashMap<String, MediaSemantics>,
    retained: &mut Vec<ObjectRef>,
    policy: CompactionPreflightPolicy,
) -> Result<ToolResultItem, MediaPreflightError> {
    let image_part_count = result
        .images
        .iter()
        .filter(|part| matches!(part, ContentPart::Image { .. }))
        .count();
    if image_part_count == 0 {
        return Ok(result);
    }
    let sites = collect_sites(&result.images, store, retained, policy)?;
    if sites.is_empty() {
        if policy == CompactionPreflightPolicy::Strict {
            return Err(MediaPreflightError::Enrichment(
                "tool result image could not be decoded from the conversation snapshot".to_string(),
            ));
        }
        // best_effort: leave the images for the sanitizer's placeholder path.
        return Ok(result);
    }
    let analysis = analyze_sites(backend, sites, digest_to_semantics, policy).await?;
    if policy == CompactionPreflightPolicy::Strict
        && (analysis.len() != image_part_count || analysis.iter().any(|(_, s)| s.is_none()))
    {
        return Err(MediaPreflightError::Enrichment(
            "tool result image semantics could not be produced".to_string(),
        ));
    }
    let envelope_parts: Vec<String> = analysis
        .into_iter()
        .filter_map(|(_, semantics)| semantics.map(|s| render_semantics_envelope(&s)))
        .collect();
    let mut result = result;
    result.images.clear();
    if !envelope_parts.is_empty() {
        let mut content = result.content.as_ref().to_string();
        for envelope in envelope_parts {
            content.push_str("\n\n");
            content.push_str(&envelope);
        }
        result.content = Arc::<str>::from(content);
    }
    Ok(result)
}

/// Collect image sites (persisting source blobs and recording the written
/// objects in `retained`) from a part list, in order.
fn collect_sites(
    parts: &[ContentPart],
    store: &MediaArtifactStore,
    retained: &mut Vec<ObjectRef>,
    policy: CompactionPreflightPolicy,
) -> Result<Vec<ImageSite>, MediaPreflightError> {
    let mut sites = Vec::new();
    for (part_index, part) in parts.iter().enumerate() {
        let ContentPart::Image { url } = part else {
            continue;
        };
        let Some(bytes) = parse_data_url(url) else {
            continue;
        };
        match store.put_blob(&bytes) {
            Ok(digest) => {
                sites.push(ImageSite {
                    part_index,
                    digest: digest.clone(),
                });
                retained.push(ObjectRef {
                    kind: ArtifactKind::Blob,
                    hash: digest,
                });
            }
            Err(error) => {
                if policy == CompactionPreflightPolicy::Strict {
                    return Err(MediaPreflightError::Store(error.to_string()));
                }
                // best_effort: leave this part as-is (sanitizer placeholder).
            }
        }
    }
    Ok(sites)
}

/// Resolve semantics for each site, reusing per-preflight digest cache
/// entries and issuing bounded backend requests for the rest.
///
/// Returns `(part_index, Option<semantics>)` in site order. `None` entries
/// are uncached sites whose analysis failed (best_effort only; `strict`
/// propagates the error instead).
async fn analyze_sites<B: CompactionAnalyzer + ?Sized>(
    backend: &B,
    sites: Vec<ImageSite>,
    digest_to_semantics: &mut HashMap<String, MediaSemantics>,
    policy: CompactionPreflightPolicy,
) -> Result<Vec<(usize, Option<MediaSemantics>)>, MediaPreflightError> {
    let mut result: Vec<(usize, String, Option<MediaSemantics>)> = Vec::with_capacity(sites.len());
    let mut fresh: Vec<ImageSite> = Vec::new();
    for site in sites {
        if let Some(semantics) = digest_to_semantics.get(&site.digest).cloned() {
            result.push((site.part_index, site.digest, Some(semantics)));
        } else {
            result.push((site.part_index, site.digest.clone(), None));
            fresh.push(site);
        }
    }
    if fresh.is_empty() {
        return Ok(result
            .into_iter()
            .map(|(part_index, _, semantics)| (part_index, semantics))
            .collect());
    }

    for batch in fresh.chunks(MAX_IMAGES_PER_REQUEST) {
        let request = MediaUnderstandingRequest {
            media: batch
                .iter()
                .map(|site| MediaSource::ArtifactRef {
                    blob_hash: site.digest.clone(),
                })
                .collect(),
            category: MediaCategory::Image,
            instruction: Some(COMPACTION_ENRICH_INSTRUCTION.to_string()),
            detail: MediaDetailLevel::Medium,
            focus: vec![],
        };
        match backend.analyze_for_compaction(request).await {
            Ok(response) => {
                if response.results.len() != batch.len() {
                    if policy == CompactionPreflightPolicy::Strict {
                        return Err(MediaPreflightError::Enrichment(format!(
                            "backend returned {} results for {} media items",
                            response.results.len(),
                            batch.len()
                        )));
                    }
                    // best_effort: adopt whatever is present, keyed by source
                    // digest so pairing by request order is never assumed.
                    for semantics in response.results {
                        if let MediaSource::ArtifactRef { blob_hash } = &semantics.source {
                            digest_to_semantics.insert(blob_hash.clone(), semantics);
                        }
                    }
                    continue;
                }
                for (site, semantics) in batch.iter().zip(response.results.iter()) {
                    digest_to_semantics.insert(site.digest.clone(), semantics.clone());
                }
            }
            Err(error) => {
                if policy == CompactionPreflightPolicy::Strict {
                    return Err(MediaPreflightError::Backend(error.to_string()));
                }
                // best_effort: leave these sites as placeholders.
            }
        }
    }

    let out = result
        .into_iter()
        .map(|(part_index, digest, semantics)| {
            let semantics = semantics.or_else(|| digest_to_semantics.get(&digest).cloned());
            (part_index, semantics)
        })
        .collect();
    Ok(out)
}

/// Parse a `data:<mime>;base64,<payload>` URL into raw bytes. `None` for any
/// other URL shape; the sanitizer keeps the placeholder path for those.
fn parse_data_url(url: &str) -> Option<Vec<u8>> {
    let payload = url.strip_prefix("data:")?.rsplit_once(";base64,")?.1;
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::session::media::consent::MediaConsentProvider;
    use xai_grok_tools::media::backend::MediaProvenance;
    use xai_grok_tools::media::domain::{MediaCategoryStrategy, MediaTransportCapabilities};

    // ── Stub analyzer ─────────────────────────────────────────────────────

    struct StubAnalyzer {
        calls: AtomicUsize,
        fail: bool,
    }

    #[async_trait]
    impl CompactionAnalyzer for StubAnalyzer {
        async fn analyze_for_compaction(
            &self,
            request: MediaUnderstandingRequest,
        ) -> Result<MediaUnderstandingResult, MediaUnderstandingError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(MediaUnderstandingError::AllRoutesExhausted(
                    "stub failure".to_string(),
                ));
            }
            let mut results = Vec::with_capacity(request.media.len());
            for source in &request.media {
                let digest = match source {
                    MediaSource::ArtifactRef { blob_hash } => blob_hash.clone(),
                    _ => String::new(),
                };
                results.push(MediaSemantics {
                    source: source.clone(),
                    category: MediaCategory::Image,
                    text: format!("semantics for {digest}"),
                    provenance: MediaProvenance {
                        provider: "stub".to_string(),
                        model: "stub-model".to_string(),
                        strategy: MediaCategoryStrategy::Native,
                    },
                });
            }
            Ok(MediaUnderstandingResult {
                results,
                attempts: vec![],
            })
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    fn data_url(seed: u8) -> String {
        let bytes = [seed; 64];
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }

    fn analyzer(fail: bool) -> StubAnalyzer {
        StubAnalyzer {
            calls: AtomicUsize::new(0),
            fail,
        }
    }

    fn user_with_image(text: &str, seed: u8) -> ConversationItem {
        let mut user = ConversationItem::user(text);
        user.add_image(data_url(seed));
        user
    }

    fn tool_result_with_image(id: &str, seed: u8) -> ConversationItem {
        ConversationItem::tool_result_with_images(
            id,
            "tool text",
            vec![ContentPart::Image {
                url: Arc::<str>::from(data_url(seed)),
            }],
        )
    }

    fn item_has_image_parts(item: &ConversationItem) -> bool {
        match item {
            ConversationItem::User(user) => user
                .content
                .iter()
                .any(|part| matches!(part, ContentPart::Image { .. })),
            ConversationItem::ToolResult(result) => !result.images.is_empty(),
            _ => false,
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn compaction_preflight_replaces_images_with_scrubbed_envelopes() {
        let tmp = tempfile::tempdir().unwrap();
        let analyzer = analyzer(false);
        let snapshot = vec![
            ConversationItem::system("sys"),
            user_with_image("look at this", 1),
            tool_result_with_image("tc-1", 2),
        ];
        let prepared = prepare_media_semantics(
            &analyzer,
            tmp.path(),
            snapshot,
            CompactionPreflightPolicy::BestEffort,
        )
        .await
        .unwrap();

        assert_eq!(prepared.enriched.len(), 3, "item count preserved");

        match &prepared.enriched[1] {
            ConversationItem::User(user) => {
                assert!(
                    user.content
                        .iter()
                        .all(|p| matches!(p, ContentPart::Text { .. })),
                    "user image part replaced by a text envelope"
                );
                assert!(
                    user.content
                        .iter()
                        .any(|p| matches!(p, ContentPart::Text { text } if text.contains("<media_semantics"))),
                    "user envelope present"
                );
            }
            other => panic!("expected user item, got {other:?}"),
        }

        match &prepared.enriched[2] {
            ConversationItem::ToolResult(result) => {
                assert_eq!(result.tool_call_id, "tc-1", "tool call id preserved");
                assert!(result.is_error.is_none());
                assert!(result.images.is_empty(), "tool result images cleared");
                assert!(
                    result.content.contains("<media_semantics"),
                    "envelope rides in tool result content"
                );
                assert!(
                    result.content.starts_with("tool text"),
                    "original tool result content preserved"
                );
            }
            other => panic!("expected tool result, got {other:?}"),
        }

        assert!(
            !xai_chat_state::compaction_utils::conversation_contains_images(&prepared.enriched),
            "no image parts remain after a correct preflight"
        );
        // The sanitizer safety net is a no-op on the correctly enriched
        // conversation.
        let sanitized =
            xai_chat_state::compaction_utils::sanitize_compaction_images(prepared.enriched.clone());
        assert_eq!(
            serde_json::to_value(&sanitized).unwrap(),
            serde_json::to_value(&prepared.enriched).unwrap(),
            "sanitizer is a no-op on correctly enriched input"
        );
    }

    #[tokio::test]
    async fn compaction_preflight_calls_backend_once_per_distinct_image() {
        let tmp = tempfile::tempdir().unwrap();
        let analyzer = analyzer(false);
        let shared = data_url(7);
        let mut user_a = ConversationItem::user("a");
        user_a.add_image(shared.clone());
        let mut user_b = ConversationItem::user("b");
        user_b.add_image(shared.clone());

        let snapshot = vec![user_a, user_b];
        let prepared = prepare_media_semantics(
            &analyzer,
            tmp.path(),
            snapshot,
            CompactionPreflightPolicy::BestEffort,
        )
        .await
        .unwrap();

        assert_eq!(
            analyzer.calls.load(Ordering::SeqCst),
            1,
            "identical image bytes are deduplicated across items within one job"
        );
        assert_eq!(prepared.enriched.len(), 2);
        for item in &prepared.enriched {
            assert!(
                !xai_chat_state::compaction_utils::conversation_contains_images(
                    std::slice::from_ref(item)
                ),
                "both items enriched"
            );
        }
    }

    #[tokio::test]
    async fn compaction_preflight_refs_source_artifacts_under_compaction_namespace() {
        let tmp = tempfile::tempdir().unwrap();
        let analyzer = analyzer(false);
        let snapshot = vec![user_with_image("u", 1)];
        let prepared = prepare_media_semantics(
            &analyzer,
            tmp.path(),
            snapshot,
            CompactionPreflightPolicy::BestEffort,
        )
        .await
        .unwrap();

        // The source artifacts that entered the compaction lifecycle (plan
        // 11.3) are referenced under refs/compaction/fp-<fingerprint>.json.
        let store = MediaArtifactStore::open(tmp.path()).unwrap();
        let refs = store.list_refs(RefKind::Compaction).unwrap();
        let fp_ref = refs
            .iter()
            .find(|entry| entry.name == format!("fp-{}", prepared.snapshot_fingerprint))
            .expect("compaction fp ref must exist after a preflight");
        let expected_hash = blake3::hash(&[1u8; 64]).to_hex().to_string();
        assert!(
            fp_ref
                .objects
                .iter()
                .any(|o| o.kind == ArtifactKind::Blob && o.hash == expected_hash),
            "the preflight source blob must be referenced under refs/compaction"
        );
    }

    #[tokio::test]
    async fn compaction_preflight_reuse_across_ladder_stages_adds_no_backend_calls() {
        let tmp = tempfile::tempdir().unwrap();
        let analyzer = analyzer(false);
        let snapshot = vec![ConversationItem::system("sys"), user_with_image("u", 1)];
        let prepared = prepare_media_semantics(
            &analyzer,
            tmp.path(),
            snapshot,
            CompactionPreflightPolicy::BestEffort,
        )
        .await
        .unwrap();
        let calls_after_preflight = analyzer.calls.load(Ordering::SeqCst);

        // Every input-ladder stage derives from the ONE enriched snapshot.
        let verbatim =
            xai_chat_state::compaction_utils::prepare_conversation_for_verbatim_summarization(
                prepared.enriched.clone(),
                true,
            );
        let lossy = xai_chat_state::compaction_utils::prepare_conversation_for_summarization(
            prepared.enriched.clone(),
        );
        let _fitted =
            xai_chat_state::compaction_utils::fit_conversation_to_budget(verbatim, 1_000_000);
        let _ = lossy;

        assert_eq!(
            analyzer.calls.load(Ordering::SeqCst),
            calls_after_preflight,
            "ladder reuse adds no backend calls"
        );
    }

    #[tokio::test]
    async fn compaction_preflight_fingerprint_reflects_raw_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let analyzer = analyzer(false);
        let mut user = ConversationItem::user("raw text");
        user.add_image(data_url(1));
        let snapshot = vec![user.clone()];
        let raw_fingerprint = fingerprint_snapshot(&snapshot);
        let prepared = prepare_media_semantics(
            &analyzer,
            tmp.path(),
            snapshot,
            CompactionPreflightPolicy::BestEffort,
        )
        .await
        .unwrap();
        assert_eq!(
            prepared.snapshot_fingerprint, raw_fingerprint,
            "fingerprint stays on the raw snapshot, not the enriched one"
        );
    }

    #[test]
    fn compaction_preflight_fingerprint_stable_for_same_snapshot_differs_when_edited() {
        let a = vec![ConversationItem::user("same")];
        let b = vec![ConversationItem::user("same")];
        assert_eq!(fingerprint_snapshot(&a), fingerprint_snapshot(&b));
        let edited = vec![ConversationItem::user("edited")];
        assert_ne!(
            fingerprint_snapshot(&a),
            fingerprint_snapshot(&edited),
            "a changed snapshot yields a different fingerprint (stale rejection)"
        );
    }

    #[tokio::test]
    async fn compaction_preflight_does_not_mutate_the_callers_raw_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let analyzer = analyzer(false);
        let mut user = ConversationItem::user("hello");
        user.add_image(data_url(1));
        let snapshot = vec![user];
        let original = snapshot.clone();
        let prepared = prepare_media_semantics(
            &analyzer,
            tmp.path(),
            snapshot,
            CompactionPreflightPolicy::BestEffort,
        )
        .await
        .unwrap();
        // The caller's raw snapshot is untouched; only the preflight copy is
        // transformed.
        assert!(
            item_has_image_parts(&original[0]),
            "caller's raw snapshot keeps its image parts"
        );
        assert_eq!(prepared.enriched.len(), original.len());
        assert!(
            !item_has_image_parts(&prepared.enriched[0]),
            "the preflight copy is enriched"
        );
    }

    #[tokio::test]
    async fn compaction_preflight_best_effort_keeps_placeholders_when_backend_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let analyzer = analyzer(true);
        let snapshot = vec![user_with_image("u", 1)];
        let prepared = prepare_media_semantics(
            &analyzer,
            tmp.path(),
            snapshot,
            CompactionPreflightPolicy::BestEffort,
        )
        .await
        .unwrap();
        assert!(
            item_has_image_parts(&prepared.enriched[0]),
            "best_effort preserves the image parts; the sanitizer placeholders them"
        );
    }

    #[tokio::test]
    async fn compaction_preflight_strict_fails_when_required_semantics_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let analyzer = analyzer(true);
        let snapshot = vec![user_with_image("u", 1)];
        let error = prepare_media_semantics(
            &analyzer,
            tmp.path(),
            snapshot,
            CompactionPreflightPolicy::Strict,
        )
        .await
        .unwrap_err();
        assert!(
            error.to_string().contains("preflight"),
            "strict surfaces a preflight error, got {error}"
        );
    }

    #[tokio::test]
    async fn compaction_preflight_strict_fails_for_undecodable_images() {
        let tmp = tempfile::tempdir().unwrap();
        let analyzer = analyzer(false);
        let mut user = ConversationItem::user("u");
        // A non-data URL cannot be analyzed; strict must reject it.
        user.add_image("https://example.test/photo.png");
        let snapshot = vec![user];
        let error = prepare_media_semantics(
            &analyzer,
            tmp.path(),
            snapshot,
            CompactionPreflightPolicy::Strict,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, MediaPreflightError::Enrichment(_)));

        // best_effort keeps the image part (placeholder path).
        let prepared = prepare_media_semantics(
            &analyzer,
            tmp.path(),
            vec![ConversationItem::user_with_parts(vec![
                ContentPart::Text {
                    text: Arc::<str>::from("u"),
                },
                ContentPart::Image {
                    url: Arc::<str>::from("https://example.test/photo.png"),
                },
            ])],
            CompactionPreflightPolicy::BestEffort,
        )
        .await
        .unwrap();
        assert!(
            xai_chat_state::compaction_utils::conversation_contains_images(&prepared.enriched),
            "best_effort keeps undecodable images for the sanitizer"
        );
    }

    #[tokio::test]
    async fn compaction_preflight_consent_denied_falls_back_to_placeholders() {
        use crate::session::media::consent::{
            ConsentDecision, ConsentRequest, DisclosurePurpose, MediaConsentProvider,
        };

        // A stub consent provider that denies the Compaction purpose for
        // every provider (and would allow explicit-tool requests). Because
        // the preflight delegates with DisclosurePurpose::Compaction, the
        // deny is honored before any bytes leave — proving the purpose key is
        // used.
        struct DenyCompactionProvider;
        #[async_trait::async_trait]
        impl MediaConsentProvider for DenyCompactionProvider {
            async fn check(&self, request: ConsentRequest) -> ConsentDecision {
                if request.purpose == DisclosurePurpose::Compaction {
                    ConsentDecision::Deny
                } else {
                    ConsentDecision::Allow
                }
            }
        }

        let tmp = tempfile::tempdir().unwrap();
        let session_dir = tmp.path().join("s");
        let context = backend_context(
            base_config(),
            default_models(),
            tmp.path().to_path_buf(),
            session_dir.clone(),
            Some(std::sync::Arc::new(DenyCompactionProvider)),
            None,
        );
        let backend = ShellMediaUnderstandingBackend::new(context).unwrap();
        let snapshot = vec![user_with_image("u", 1)];

        // best_effort: deny -> placeholder path, preflight still succeeds.
        let prepared = prepare_media_semantics(
            &backend,
            &session_dir,
            snapshot.clone(),
            CompactionPreflightPolicy::BestEffort,
        )
        .await
        .unwrap();
        assert!(
            item_has_image_parts(&prepared.enriched[0]),
            "consent denial preserves the placeholder path under best_effort"
        );

        // strict: deny -> fail closed.
        let error = prepare_media_semantics(
            &backend,
            &session_dir,
            snapshot,
            CompactionPreflightPolicy::Strict,
        )
        .await;
        assert!(
            error.is_err(),
            "consent denial must fail a strict preflight"
        );
    }

    #[test]
    fn compaction_enrich_kill_switch_env_var() {
        let previous = std::env::var_os("GROK_DISABLE_MEDIA_COMPACTION_ENRICH");
        unsafe {
            std::env::set_var("GROK_DISABLE_MEDIA_COMPACTION_ENRICH", "1");
        }
        assert!(compaction_enrich_kill_switched());
        unsafe {
            std::env::remove_var("GROK_DISABLE_MEDIA_COMPACTION_ENRICH");
        }
        assert!(!compaction_enrich_kill_switched());
        match previous {
            Some(value) => unsafe {
                std::env::set_var("GROK_DISABLE_MEDIA_COMPACTION_ENRICH", value);
            },
            None => {}
        }
    }

    #[test]
    fn compaction_enrichment_mode_gates_on_config_and_kill_switch() {
        let config = || {
            let mut config = base_config();
            config.enabled = true;
            config.compaction_enrichment = true;
            config
        };

        assert_eq!(
            compaction_enrichment_mode(None, false),
            CompactionEnrichmentMode::Disabled
        );
        assert_eq!(
            compaction_enrichment_mode(Some(&config()), true),
            CompactionEnrichmentMode::Disabled
        );

        let mut disabled = config();
        disabled.enabled = false;
        assert_eq!(
            compaction_enrichment_mode(Some(&disabled), false),
            CompactionEnrichmentMode::Disabled
        );

        let mut no_enrich = config();
        no_enrich.compaction_enrichment = false;
        assert_eq!(
            compaction_enrichment_mode(Some(&no_enrich), false),
            CompactionEnrichmentMode::Disabled
        );

        let mut strict = config();
        strict.compaction_preflight_policy = CompactionPreflightPolicy::Strict;
        assert_eq!(
            compaction_enrichment_mode(Some(&strict), false),
            CompactionEnrichmentMode::Enabled {
                policy: CompactionPreflightPolicy::Strict
            }
        );
        assert_eq!(
            compaction_enrichment_mode(Some(&config()), false),
            CompactionEnrichmentMode::Enabled {
                policy: CompactionPreflightPolicy::BestEffort
            }
        );
    }

    // ── Real-backend test scaffolding (mirrors media/backend.rs tests) ────

    fn model_entry(model: &str) -> crate::agent::config::ModelEntry {
        let mut info = crate::agent::config::ModelInfo::fallback(model);
        info.base_url = "https://example.test/v1".to_string();
        info.media_capabilities = xai_grok_tools::media::domain::MediaCapabilities {
            image: xai_grok_tools::media::domain::MediaModalitySupport::Supported,
            ..Default::default()
        };
        info.media_transport = MediaTransportCapabilities {
            image_inline: true,
            json_schema: true,
            ..Default::default()
        };
        crate::agent::config::ModelEntry {
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

    fn base_config() -> crate::agent::config::ResolvedMediaUnderstandingConfig {
        crate::agent::config::ResolvedMediaUnderstandingConfig {
            enabled: true,
            auto_enrich: false,
            compaction_enrichment: true,
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

    fn default_models() -> indexmap::IndexMap<String, crate::agent::config::ModelEntry> {
        let mut models = indexmap::IndexMap::new();
        models.insert("vision-model".to_string(), model_entry("vision-model"));
        models
    }

    fn build_manager(
        models: indexmap::IndexMap<String, crate::agent::config::ModelEntry>,
    ) -> crate::agent::models::ModelsManager {
        let tmp = std::env::temp_dir().join(format!(
            "grok-test-media-compaction-{}",
            uuid::Uuid::new_v4()
        ));
        let auth_manager = std::sync::Arc::new(crate::auth::AuthManager::new(
            &tmp,
            crate::auth::GrokComConfig::default(),
        ));
        crate::agent::models::ModelsManager::new(
            None,
            models,
            agent_client_protocol::ModelId::new("default"),
            auth_manager,
            crate::agent::config::Config::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn backend_context(
        config: crate::agent::config::ResolvedMediaUnderstandingConfig,
        models: indexmap::IndexMap<String, crate::agent::config::ModelEntry>,
        workspace_root: std::path::PathBuf,
        session_dir: std::path::PathBuf,
        consent: Option<std::sync::Arc<dyn MediaConsentProvider>>,
        current_auth: Option<crate::auth::GrokAuth>,
    ) -> super::super::backend::ShellMediaBackendContext {
        use super::super::backend::InvokerCredentialSnapshot;
        super::super::backend::ShellMediaBackendContext {
            config,
            models: build_manager(models),
            auth: None,
            current_auth,
            session_dir,
            workspace_root,
            permission: Some(xai_grok_workspace::permission::PermissionHandle::allow_all()),
            session_id: Some("media-compaction-test".to_string()),
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
}
