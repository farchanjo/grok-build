//! Purpose-scoped external-disclosure consent gate (plan sections 7.2 and 16).
//!
//! This is the **second** host gate, deliberately separate from
//! filesystem/tool permission. It is **YOLO-proof**: always-approve / YOLO
//! mode only affects the permission handle (which returns `Allow` for
//! everything); the consent gate consults its own purpose-scoped provider and
//! is never bypassed by the permission mode.
//!
//! Consent is consulted **before every fallback provider transmission**: a
//! route that would move bytes to a different provider than an earlier route
//! must obtain its own consent for `(provider_identity, category, purpose)`.
//!
//! When no consent provider is injected the gate **denies** (no external
//! disclosure without consent). Managed policy may tighten or deny consent;
//! the gate never loosens it.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use xai_grok_tools::implementations::grok_build::ask_user_question::types::{
    UserQuestionRequest, UserQuestionResponse,
};
use xai_grok_tools::implementations::grok_build::ask_user_question::{Question, QuestionOption};
use xai_grok_tools::media::domain::MediaCategory;

/// Purpose of a media-understanding request.
///
/// Mirrors the ledger `UsagePurpose` but stays independent so the consent
/// gate's record keys do not couple to ledger internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DisclosurePurpose {
    /// Explicit `analyze_media` tool request.
    ExplicitTool,
    /// Automatic attachment enrichment for a text-only session model.
    AutoAttachment,
    /// Compaction preflight enrichment.
    Compaction,
}

/// Outcome of a consent check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConsentDecision {
    /// Consent explicitly granted for this exact disclosure.
    Allow,
    /// Consent not granted or unavailable; the route must be skipped without
    /// sending bytes.
    Deny,
}

/// What the consent provider is asked about.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ConsentRequest {
    /// Stable provider identity (`"xai"`, `"openrouter"`, `"openai"`,
    /// `"anthropic"`, `"custom"`).
    pub provider_identity: String,
    pub category: MediaCategory,
    pub purpose: DisclosurePurpose,
}

/// Host-side consent provider seam.
///
/// The interactive implementation (PR 9) persists purpose-scoped consent
/// decisions and renders the disclosure prompt; PR 6 ships the gate structure
/// and the fail-closed default (no provider ⇒ no consent ⇒ skip).
#[async_trait::async_trait]
pub(crate) trait MediaConsentProvider: Send + Sync + 'static {
    async fn check(&self, request: ConsentRequest) -> ConsentDecision;
}

/// YOLO-proof purpose-scoped consent gate.
///
/// Deliberately not `Debug`: the injected `MediaConsentProvider` is a trait
/// object without a `Debug` supertrait, so no debug formatting of the gate is
/// possible (or needed).
pub(crate) struct DisclosureConsentGate {
    provider: Option<std::sync::Arc<dyn MediaConsentProvider>>,
}

impl DisclosureConsentGate {
    pub(crate) fn new(provider: Option<std::sync::Arc<dyn MediaConsentProvider>>) -> Self {
        Self { provider }
    }

    /// Check consent for one provider/category/purpose transmission.
    ///
    /// `None` provider fails closed: no external disclosure without consent.
    pub(crate) async fn check(&self, request: ConsentRequest) -> ConsentDecision {
        match &self.provider {
            Some(provider) => provider.check(request).await,
            None => ConsentDecision::Deny,
        }
    }
}

/// Stable snake_case provider label used as the consent key and in ledger
/// rows.
pub(crate) fn provider_identity_str(
    identity: xai_grok_inference::config::ProviderIdentity,
) -> String {
    use xai_grok_inference::config::ProviderIdentity as P;
    match identity {
        P::Xai => "xai".to_string(),
        P::OpenAi => "openai".to_string(),
        P::OpenRouter => "openrouter".to_string(),
        P::Anthropic => "anthropic".to_string(),
        P::Custom => "custom".to_string(),
    }
}

/// Version prefix of the persisted consent-key encoding. Bump only when the
/// encoding changes incompatibly; [`canonical_consent_key`] migrates older
/// rows on load.
const CONSENT_KEY_VERSION: &str = "v1";

/// Stable snake_case category label used inside consent keys.
///
/// Matches the serde `rename_all = "snake_case"` wire spelling of
/// [`MediaCategory`], spelled out so the persisted encoding never depends on
/// a derive attribute changing.
fn category_key(category: MediaCategory) -> &'static str {
    match category {
        MediaCategory::Auto => "auto",
        MediaCategory::Image => "image",
        MediaCategory::Audio => "audio",
        MediaCategory::Video => "video",
    }
}

/// Stable snake_case purpose label used inside consent keys.
fn purpose_key(purpose: DisclosurePurpose) -> &'static str {
    match purpose {
        DisclosurePurpose::ExplicitTool => "explicit_tool",
        DisclosurePurpose::AutoAttachment => "auto_attachment",
        DisclosurePurpose::Compaction => "compaction",
    }
}

/// Reverse of [`category_key`]; `None` for unknown labels.
fn parse_category_key(label: &str) -> Option<MediaCategory> {
    match label {
        "auto" => Some(MediaCategory::Auto),
        "image" => Some(MediaCategory::Image),
        "audio" => Some(MediaCategory::Audio),
        "video" => Some(MediaCategory::Video),
        _ => None,
    }
}

/// Reverse of [`purpose_key`]; `None` for unknown labels.
fn parse_purpose_key(label: &str) -> Option<DisclosurePurpose> {
    match label {
        "explicit_tool" => Some(DisclosurePurpose::ExplicitTool),
        "auto_attachment" => Some(DisclosurePurpose::AutoAttachment),
        "compaction" => Some(DisclosurePurpose::Compaction),
        _ => None,
    }
}

/// Parse the `Debug`-formatted category spelling written by pre-versioning
/// binaries (`Image`, `Audio`, `Video`, `Auto`).
fn parse_legacy_category(label: &str) -> Option<MediaCategory> {
    match label {
        "Auto" => Some(MediaCategory::Auto),
        "Image" => Some(MediaCategory::Image),
        "Audio" => Some(MediaCategory::Audio),
        "Video" => Some(MediaCategory::Video),
        _ => None,
    }
}

/// Parse the `Debug`-formatted purpose spelling written by pre-versioning
/// binaries (`ExplicitTool`, `AutoAttachment`, `Compaction`).
fn parse_legacy_purpose(label: &str) -> Option<DisclosurePurpose> {
    match label {
        "ExplicitTool" => Some(DisclosurePurpose::ExplicitTool),
        "AutoAttachment" => Some(DisclosurePurpose::AutoAttachment),
        "Compaction" => Some(DisclosurePurpose::Compaction),
        _ => None,
    }
}

/// Map a persisted consent key to its canonical versioned form.
///
/// Accepts current `v1|provider|category|purpose` rows and migrates legacy
/// rows that used `Debug` formatting (`provider|Category|Purpose`). Returns
/// `None` for unrecognized keys so the row is skipped (fail closed).
fn canonical_consent_key(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.split('|').collect();
    match parts.as_slice() {
        [version, provider, category, purpose] if *version == CONSENT_KEY_VERSION => {
            if parse_category_key(category).is_none() || parse_purpose_key(purpose).is_none() {
                return None;
            }
            Some(format!("{version}|{provider}|{category}|{purpose}"))
        }
        [provider, category, purpose] => {
            let category = parse_legacy_category(category)?;
            let purpose = parse_legacy_purpose(purpose)?;
            Some(format!(
                "{CONSENT_KEY_VERSION}|{provider}|{}|{}",
                category_key(category),
                purpose_key(purpose)
            ))
        }
        _ => None,
    }
}

/// Persisted row in the purpose-scoped consent decisions file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConsentDecisionRow {
    /// Versioned consent key: `v1|provider|category|purpose` (snake_case).
    /// Legacy `provider|Category|Purpose` rows (Debug-formatted, written by
    /// pre-versioning binaries) are migrated on load.
    key: String,
    /// Whether the user granted consent for this exact disclosure.
    allow: bool,
}

/// Interactive purpose-scoped consent provider (PR 9).
///
/// - Auto-consents the session's own internal provider (`"xai"`), matching
///   the legacy image-describe path.
/// - For external providers, persists purpose-scoped decisions
///   (`v1|provider|category|purpose`, snake_case) to the session's media
///   store and asks the user via `AskUserQuestion` when no decision exists
///   yet.
/// - When no ask-user channel is available (headless, tests, compaction
///   without a client), it fails closed for external providers.
pub(crate) struct InteractiveMediaConsentProvider {
    /// Session-scoped media store directory (`<session_dir>/assets/media`).
    store_dir: PathBuf,
    /// Optional sender for `AskUserQuestion` round-trips.
    ask_user: Option<mpsc::UnboundedSender<UserQuestionRequest>>,
    /// In-memory decision cache (loaded from the decisions file at startup).
    decisions: parking_lot::Mutex<HashMap<String, bool>>,
}

impl InteractiveMediaConsentProvider {
    pub(crate) fn new(
        store_dir: PathBuf,
        ask_user: Option<mpsc::UnboundedSender<UserQuestionRequest>>,
    ) -> Self {
        let decisions = load_decision_rows(&store_dir);
        Self {
            store_dir,
            ask_user,
            decisions: parking_lot::Mutex::new(decisions),
        }
    }

    fn key(request: &ConsentRequest) -> String {
        format!(
            "{CONSENT_KEY_VERSION}|{}|{}|{}",
            request.provider_identity,
            category_key(request.category),
            purpose_key(request.purpose)
        )
    }

    fn decisions_path(&self) -> PathBuf {
        self.store_dir.join("consent.jsonl")
    }

    fn cached(&self, request: &ConsentRequest) -> Option<bool> {
        self.decisions.lock().get(&Self::key(request)).copied()
    }

    fn persist(&self, request: &ConsentRequest, allow: bool) {
        let row = ConsentDecisionRow {
            key: Self::key(request),
            allow,
        };
        self.decisions.lock().insert(row.key.clone(), allow);
        let mut line = match serde_json::to_vec(&row) {
            Ok(line) => line,
            Err(_) => return,
        };
        line.push(b'\n');
        let _ = super::append_jsonl_line_locked(&self.decisions_path(), line);
    }

    async fn ask(&self, request: &ConsentRequest) -> bool {
        let Some(sender) = self.ask_user.as_ref() else {
            return false;
        };
        let question = Question {
            question: format!(
                "Allow sending media to {} for {:?}?",
                request.provider_identity, request.purpose
            ),
            options: vec![
                QuestionOption {
                    label: "Allow".to_string(),
                    description: "Permit this disclosure for this purpose".to_string(),
                    preview: None,
                    id: None,
                },
                QuestionOption {
                    label: "Deny".to_string(),
                    description: "Skip without sending bytes".to_string(),
                    preview: None,
                    id: None,
                },
            ],
            multi_select: Some(false),
            id: Some("media_consent".to_string()),
        };
        let (result_tx, result_rx) = oneshot::channel();
        let request = UserQuestionRequest {
            tool_call_id: format!("media_consent_{}", super::now_ts()),
            questions: vec![question],
            result_tx,
        };
        if sender.send(request).is_err() {
            return false;
        }
        match result_rx.await {
            Ok(Ok(UserQuestionResponse::Accepted { answers, .. })) => answers
                .get("media_consent")
                .and_then(|values| values.first())
                .map(|label| label == "Allow")
                .unwrap_or(false),
            Ok(_) | Err(_) => false,
        }
    }
}

#[async_trait::async_trait]
impl MediaConsentProvider for InteractiveMediaConsentProvider {
    async fn check(&self, request: ConsentRequest) -> ConsentDecision {
        // First-party internal provider: auto-consented (legacy path).
        if request.provider_identity == "xai" {
            return ConsentDecision::Allow;
        }
        if let Some(allow) = self.cached(&request) {
            return if allow {
                ConsentDecision::Allow
            } else {
                ConsentDecision::Deny
            };
        }
        let allow = self.ask(&request).await;
        self.persist(&request, allow);
        if allow {
            ConsentDecision::Allow
        } else {
            ConsentDecision::Deny
        }
    }
}

/// Load persisted decision rows from the decisions file into a keyed map.
/// A corrupt/missing file yields an empty map (fail closed). Legacy rows
/// persisted with `Debug`-formatted keys are migrated to the canonical
/// versioned key on load; unrecognized rows are skipped.
fn load_decision_rows(store_dir: &PathBuf) -> HashMap<String, bool> {
    let path = store_dir.join("consent.jsonl");
    let mut out = HashMap::new();
    for line in super::read_jsonl::<ConsentDecisionRow>(&path).unwrap_or_default() {
        match canonical_consent_key(&line.key) {
            Some(key) => {
                out.insert(key, line.allow);
            }
            None => {
                tracing::warn!(
                    key = %line.key,
                    "skipping unrecognized media consent decision row (fail closed)"
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use xai_grok_inference::config::ProviderIdentity;

    /// Test-only provider that allows exactly the granted combos.
    struct StubConsentProvider {
        allowed: Vec<(String, MediaCategory, DisclosurePurpose)>,
    }

    #[async_trait::async_trait]
    impl MediaConsentProvider for StubConsentProvider {
        async fn check(&self, request: ConsentRequest) -> ConsentDecision {
            let key = (request.provider_identity, request.category, request.purpose);
            if self.allowed.contains(&key) {
                ConsentDecision::Allow
            } else {
                ConsentDecision::Deny
            }
        }
    }

    fn request(
        provider: &str,
        category: MediaCategory,
        purpose: DisclosurePurpose,
    ) -> ConsentRequest {
        ConsentRequest {
            provider_identity: provider.to_string(),
            category,
            purpose,
        }
    }

    #[tokio::test]
    async fn media_consent_fails_closed_without_provider() {
        let gate = DisclosureConsentGate::new(None);
        assert_eq!(
            gate.check(request(
                "openrouter",
                MediaCategory::Image,
                DisclosurePurpose::ExplicitTool
            ))
            .await,
            ConsentDecision::Deny,
            "no consent provider must deny every disclosure"
        );
    }

    #[tokio::test]
    async fn media_consent_is_purpose_scoped() {
        let provider = Arc::new(StubConsentProvider {
            allowed: vec![(
                "openrouter".to_string(),
                MediaCategory::Image,
                DisclosurePurpose::ExplicitTool,
            )],
        }) as Arc<dyn MediaConsentProvider>;
        let gate = DisclosureConsentGate::new(Some(provider));

        assert_eq!(
            gate.check(request(
                "openrouter",
                MediaCategory::Image,
                DisclosurePurpose::ExplicitTool
            ))
            .await,
            ConsentDecision::Allow
        );
        // Same provider+category but a different purpose must not inherit the
        // explicit-tool grant.
        assert_eq!(
            gate.check(request(
                "openrouter",
                MediaCategory::Image,
                DisclosurePurpose::Compaction
            ))
            .await,
            ConsentDecision::Deny
        );
        // Different category must not inherit.
        assert_eq!(
            gate.check(request(
                "openrouter",
                MediaCategory::Video,
                DisclosurePurpose::ExplicitTool
            ))
            .await,
            ConsentDecision::Deny
        );
    }

    #[tokio::test]
    async fn media_consent_checked_per_fallback_provider() {
        let provider = Arc::new(StubConsentProvider {
            allowed: vec![
                (
                    "xai".to_string(),
                    MediaCategory::Image,
                    DisclosurePurpose::ExplicitTool,
                ),
                (
                    "openrouter".to_string(),
                    MediaCategory::Image,
                    DisclosurePurpose::ExplicitTool,
                ),
            ],
        }) as Arc<dyn MediaConsentProvider>;
        let gate = DisclosureConsentGate::new(Some(provider));

        // Primary (xai) consented, fallback (openrouter) consented.
        assert_eq!(
            gate.check(request(
                "xai",
                MediaCategory::Image,
                DisclosurePurpose::ExplicitTool
            ))
            .await,
            ConsentDecision::Allow
        );
        assert_eq!(
            gate.check(request(
                "openrouter",
                MediaCategory::Image,
                DisclosurePurpose::ExplicitTool
            ))
            .await,
            ConsentDecision::Allow
        );
        // A third fallback provider not covered by consent must be denied
        // even though the user never asked about it.
        assert_eq!(
            gate.check(request(
                "anthropic",
                MediaCategory::Image,
                DisclosurePurpose::ExplicitTool
            ))
            .await,
            ConsentDecision::Deny
        );
    }

    #[tokio::test]
    async fn media_consent_not_bypassed_by_permission_mode() {
        // YOLO/always-approve only changes the permission handle; the consent
        // gate owns its own decision and stays closed for unconsented
        // providers regardless of any "allow everything" permission posture.
        let provider =
            Arc::new(StubConsentProvider { allowed: vec![] }) as Arc<dyn MediaConsentProvider>;
        let gate = DisclosureConsentGate::new(Some(provider));
        assert_eq!(
            gate.check(request(
                "openrouter",
                MediaCategory::Image,
                DisclosurePurpose::AutoAttachment
            ))
            .await,
            ConsentDecision::Deny
        );
    }

    #[test]
    fn media_consent_provider_label_is_stable() {
        assert_eq!(provider_identity_str(ProviderIdentity::Xai), "xai");
        assert_eq!(
            provider_identity_str(ProviderIdentity::OpenRouter),
            "openrouter"
        );
        assert_eq!(provider_identity_str(ProviderIdentity::OpenAi), "openai");
        assert_eq!(
            provider_identity_str(ProviderIdentity::Anthropic),
            "anthropic"
        );
        assert_eq!(provider_identity_str(ProviderIdentity::Custom), "custom");
    }

    #[test]
    fn media_consent_purpose_round_trips() {
        for purpose in [
            DisclosurePurpose::ExplicitTool,
            DisclosurePurpose::AutoAttachment,
            DisclosurePurpose::Compaction,
        ] {
            let json = serde_json::to_value(&purpose).unwrap();
            let back: DisclosurePurpose = serde_json::from_value(json).unwrap();
            assert_eq!(back, purpose);
        }
    }

    #[test]
    fn media_consent_keys_are_versioned_and_stable() {
        // Exact persisted encoding: versioned, snake_case, no Debug output.
        assert_eq!(
            InteractiveMediaConsentProvider::key(&request(
                "openrouter",
                MediaCategory::Image,
                DisclosurePurpose::ExplicitTool
            )),
            "v1|openrouter|image|explicit_tool"
        );
        assert_eq!(
            InteractiveMediaConsentProvider::key(&request(
                "xai",
                MediaCategory::Audio,
                DisclosurePurpose::AutoAttachment
            )),
            "v1|xai|audio|auto_attachment"
        );
        assert_eq!(
            InteractiveMediaConsentProvider::key(&request(
                "anthropic",
                MediaCategory::Video,
                DisclosurePurpose::Compaction
            )),
            "v1|anthropic|video|compaction"
        );
        // Debug-formatted spellings must never appear in persisted keys.
        let key = InteractiveMediaConsentProvider::key(&request(
            "openrouter",
            MediaCategory::Image,
            DisclosurePurpose::ExplicitTool,
        ));
        for debug_spelling in ["Image", "ExplicitTool", "AutoAttachment", "Compaction"] {
            assert!(
                !key.contains(debug_spelling),
                "persisted key must not use Debug spelling `{debug_spelling}`: {key}"
            );
        }
    }

    /// Legacy rows persisted with `Debug`-formatted keys (pre-versioning) are
    /// still honored and migrated to the versioned encoding on load.
    #[tokio::test]
    async fn media_consent_legacy_debug_keys_still_work() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = tmp.path().join("assets").join("media");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(
            store.join("consent.jsonl"),
            concat!(
                r#"{"key":"openrouter|Image|ExplicitTool","allow":true}"#,
                "\n",
                r#"{"key":"anthropic|Video|AutoAttachment","allow":false}"#,
                "\n",
            ),
        )
        .unwrap();

        let provider = InteractiveMediaConsentProvider::new(store.clone(), None);

        // The legacy grant is honored without any ask-user channel.
        let granted = request(
            "openrouter",
            MediaCategory::Image,
            DisclosurePurpose::ExplicitTool,
        );
        assert_eq!(
            provider.check(granted.clone()).await,
            ConsentDecision::Allow
        );
        // The legacy denial is honored too.
        let denied = request(
            "anthropic",
            MediaCategory::Video,
            DisclosurePurpose::AutoAttachment,
        );
        assert_eq!(provider.check(denied.clone()).await, ConsentDecision::Deny);
        // An unrelated purpose is NOT covered by the migrated rows.
        let unrelated = request(
            "openrouter",
            MediaCategory::Image,
            DisclosurePurpose::Compaction,
        );
        assert_eq!(provider.check(unrelated).await, ConsentDecision::Deny);

        // New decisions persist under the versioned key (the migrated legacy
        // rows themselves are only re-keyed in memory and stay readable).
        let file = std::fs::read_to_string(store.join("consent.jsonl")).unwrap();
        assert!(
            file.contains(r#""v1|openrouter|image|compaction""#),
            "persisted rows must use the versioned key: {file}"
        );
        assert!(
            file.contains(r#"{"key":"openrouter|Image|ExplicitTool","allow":true}"#),
            "legacy rows must remain readable on disk: {file}"
        );
    }

    /// Unrecognized persisted keys are skipped (fail closed) instead of
    /// granting anything.
    #[test]
    fn media_consent_skips_unrecognized_rows() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = tmp.path().join("assets").join("media");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(
            store.join("consent.jsonl"),
            concat!(
                // Legacy row with an unknown category spelling.
                r#"{"key":"openrouter|Bogus|ExplicitTool","allow":true}"#,
                "\n",
                // Versioned row with an unknown purpose spelling.
                r#"{"key":"v1|openrouter|image|bogus_purpose","allow":true}"#,
                "\n",
                // Well-formed versioned row.
                r#"{"key":"v1|openrouter|image|explicit_tool","allow":true}"#,
                "\n",
            ),
        )
        .unwrap();

        let provider = InteractiveMediaConsentProvider::new(store, None);
        let decisions = provider.decisions.lock();
        assert_eq!(
            decisions.len(),
            1,
            "malformed rows must be skipped: {decisions:?}"
        );
        assert_eq!(
            decisions.get("v1|openrouter|image|explicit_tool"),
            Some(&true)
        );
    }

    /// Drive an ask-user round-trip with a canned response.
    async fn provider_with_answer(
        store_dir: &PathBuf,
        answer: &str,
    ) -> InteractiveMediaConsentProvider {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<UserQuestionRequest>();
        let store_dir = store_dir.clone();
        let answer = answer.to_string();
        tokio::spawn(async move {
            if let Some(req) = rx.recv().await {
                let mut answers = indexmap::IndexMap::new();
                answers.insert("media_consent".to_string(), vec![answer]);
                let _ = req.result_tx.send(Ok(UserQuestionResponse::Accepted {
                    answers,
                    annotations: None,
                }));
            }
        });
        InteractiveMediaConsentProvider::new(store_dir, Some(tx))
    }

    /// The interactive provider asks the user once, persists the decision,
    /// and reuses it on subsequent checks (no re-prompt).
    #[tokio::test]
    async fn media_consent_interactive_persists_and_reuses() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = tmp.path().join("assets").join("media");
        std::fs::create_dir_all(&store).unwrap();
        let provider = provider_with_answer(&store, "Allow").await;

        let req = request(
            "openrouter",
            MediaCategory::Image,
            DisclosurePurpose::ExplicitTool,
        );
        assert_eq!(provider.check(req.clone()).await, ConsentDecision::Allow);
        assert_eq!(provider.check(req.clone()).await, ConsentDecision::Allow);

        // Persisted decision survives a fresh provider instance (replay of
        // the decisions file) — no second prompt is needed.
        let provider2 = InteractiveMediaConsentProvider::new(store.clone(), None);
        assert_eq!(provider2.check(req.clone()).await, ConsentDecision::Allow);

        // A different purpose is NOT covered by the persisted grant.
        let other = request(
            "openrouter",
            MediaCategory::Image,
            DisclosurePurpose::Compaction,
        );
        // No ask-user channel on the fresh provider -> fails closed.
        assert_eq!(provider2.check(other).await, ConsentDecision::Deny);
    }

    /// The interactive provider auto-consents the internal provider and
    /// denies external providers when no ask-user channel is available.
    #[tokio::test]
    async fn media_consent_interactive_fails_closed_without_ask_user() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = tmp.path().join("assets").join("media");
        std::fs::create_dir_all(&store).unwrap();
        let provider = InteractiveMediaConsentProvider::new(store, None);

        assert_eq!(
            provider
                .check(request(
                    "xai",
                    MediaCategory::Image,
                    DisclosurePurpose::ExplicitTool
                ))
                .await,
            ConsentDecision::Allow
        );
        assert_eq!(
            provider
                .check(request(
                    "openrouter",
                    MediaCategory::Image,
                    DisclosurePurpose::ExplicitTool
                ))
                .await,
            ConsentDecision::Deny
        );
    }

    /// A "Deny" answer is persisted and honored on later checks.
    #[tokio::test]
    async fn media_consent_interactive_persists_deny() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = tmp.path().join("assets").join("media");
        std::fs::create_dir_all(&store).unwrap();
        let provider = provider_with_answer(&store, "Deny").await;

        let req = request(
            "anthropic",
            MediaCategory::Video,
            DisclosurePurpose::AutoAttachment,
        );
        assert_eq!(provider.check(req.clone()).await, ConsentDecision::Deny);
        assert_eq!(provider.check(req.clone()).await, ConsentDecision::Deny);
    }
}
