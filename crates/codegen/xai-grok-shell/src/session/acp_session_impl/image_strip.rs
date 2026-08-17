//! Policy for deciding when an inference-time image strip may rewrite stored
//! history.
//!
//! Only a typed server rejection with exactly one unique URL is eligible. The
//! rewrite waits for the matching request to complete successfully, is dropped
//! on matching failure, and is reported as durable only after chat-state's
//! backup-gated disk acknowledgement.

use xai_chat_state::StripOutcome;
use xai_grok_inference::{RequestId, StripReason};

use crate::extensions::notification::SessionUpdate as XaiSessionUpdate;
use crate::session::acp_session::SessionActor;

impl SessionActor {
    pub(crate) async fn handle_images_stripped(
        &self,
        request_id: RequestId,
        stripped_urls: Vec<std::sync::Arc<str>>,
        reason: StripReason,
    ) {
        let stripped = stripped_urls.len();
        let mut unique = stripped_urls;
        unique.sort();
        unique.dedup();

        let persist_deferred = reason == StripReason::ServerRejected && unique.len() == 1;
        if persist_deferred {
            *self.pending_image_strip.lock() = Some((request_id.clone(), unique));
        }

        xai_grok_telemetry::unified_log::warn(
            "shell.turn.images_stripped",
            Some(self.session_info.id.0.as_ref()),
            Some(serde_json::json!({
                "sampler_request_id": request_id.as_str(),
                "stripped": stripped,
                "reason": reason.as_str(),
                "persist_deferred": persist_deferred,
            })),
        );

        if !persist_deferred {
            self.send_xai_notification(XaiSessionUpdate::ImageDropped {
                notes: vec![format!(
                    "This request failed over its images (or was too large); \
                     {stripped} image(s) were left out of the retry."
                )],
            })
            .await;
        }
    }

    pub(crate) async fn apply_pending_image_strip(&self, request_id: &RequestId) {
        let urls = {
            let mut pending = self.pending_image_strip.lock();
            match pending.take() {
                Some((pending_id, urls)) if &pending_id == request_id => Some(urls),
                other => {
                    *pending = other;
                    None
                }
            }
        };
        let Some(urls) = urls else { return };

        let outcome = self.chat_state_handle.strip_conversation_images(urls).await;
        let (outcome_label, persisted) = match outcome {
            StripOutcome::Applied { stripped } => ("applied", stripped),
            StripOutcome::NoMatch => ("no_match", 0),
            StripOutcome::WriteFailed { .. } => ("write_failed", 0),
            StripOutcome::ActorUnavailable => ("actor_unavailable", 0),
        };
        xai_grok_telemetry::unified_log::warn(
            "shell.turn.images_strip_persisted",
            Some(self.session_info.id.0.as_ref()),
            Some(serde_json::json!({
                "sampler_request_id": request_id.as_str(),
                "outcome": outcome_label,
                "persisted": persisted,
            })),
        );

        let notes = match outcome {
            StripOutcome::Applied { .. } => vec![
                "The server could not process an image, so it was removed from \
                 the conversation. Re-attach it if it is still needed."
                    .to_string(),
            ],
            StripOutcome::NoMatch
            | StripOutcome::WriteFailed { .. }
            | StripOutcome::ActorUnavailable => vec![
                "The server could not process an image, so it was left out of \
                 this request."
                    .to_string(),
            ],
        };
        self.send_xai_notification(XaiSessionUpdate::ImageDropped { notes })
            .await;
    }

    pub(crate) fn drop_pending_image_strip(&self, request_id: &RequestId) {
        let mut pending = self.pending_image_strip.lock();
        if pending
            .as_ref()
            .is_some_and(|(pending_id, _)| pending_id == request_id)
        {
            tracing::debug!(
                sampler_request_id = request_id.as_str(),
                "dropping buffered image strip because the stripped retry failed"
            );
            *pending = None;
        }
    }
}
