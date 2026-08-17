//! Durable image-strip lifecycle policy.

use std::sync::Arc;

use super::support::*;
use super::*;
use xai_grok_inference::{
    InferenceErrorInfo, InferenceErrorKind, InferenceEvent, InferenceLatencyStats, RequestId,
    StripReason,
};
use xai_grok_inference_types::{ContentPart, ConversationItem, ConversationResponse};

const IMAGE: &str = "data:image/png;base64,KEEPME";

fn user_with_image(url: &str) -> ConversationItem {
    let mut item = ConversationItem::user("look");
    item.add_image(url);
    item
}

fn has_image(items: &[ConversationItem], url: &str) -> bool {
    items.iter().any(|item| match item {
        ConversationItem::User(user) => user
            .content
            .iter()
            .any(|part| matches!(part, ContentPart::Image { url: value } if value.as_ref() == url)),
        _ => false,
    })
}

async fn seed(actor: &SessionActor, url: &str) {
    actor
        .chat_state_handle
        .push_user_message(user_with_image(url));
    assert!(has_image(
        &actor.chat_state_handle.get_conversation().await,
        url
    ));
}

fn stripped(request_id: &RequestId, urls: &[&str], reason: StripReason) -> InferenceEvent {
    InferenceEvent::ImagesStripped {
        request_id: request_id.clone(),
        stripped_urls: urls.iter().map(|url| Arc::<str>::from(*url)).collect(),
        reason,
    }
}

fn completed(request_id: &RequestId) -> InferenceEvent {
    InferenceEvent::Completed {
        request_id: request_id.clone(),
        response: Box::new(ConversationResponse {
            items: vec![ConversationItem::assistant("recovered")],
            stop_reason: None,
            usage: None,
            cost_usd_ticks: None,
            message_chunks_emitted: 1,
            doom_loop_signals: vec![],
            stop_message: None,
            fallback_served_model: None,
        }),
        metrics: InferenceLatencyStats::default(),
    }
}

fn failed(request_id: &RequestId) -> InferenceEvent {
    InferenceEvent::Failed {
        request_id: request_id.clone(),
        error: InferenceErrorInfo {
            kind: InferenceErrorKind::Api,
            status_code: Some(400),
            message: "bad image".into(),
            is_retryable: false,
            retry_after_secs: None,
            model_metadata: None,
            diagnostics: None,
            error_code: None,
            empty_response_context: None,
            doom_loop_triggers: None,
            doom_loop_aborted_at_chunk: None,
            credential: xai_grok_inference_types::SentCredential::Unknown,
        },
    }
}

async fn settle() {
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn image_strip_heuristic_is_request_local_after_completed() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel();
            let actor =
                Arc::new(create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await);
            seed(&actor, IMAGE).await;
            let request_id = RequestId::from("heuristic");

            actor
                .handle_sampling_event(stripped(
                    &request_id,
                    &[IMAGE],
                    StripReason::PayloadHeuristic,
                ))
                .await;
            actor.handle_sampling_event(completed(&request_id)).await;
            settle().await;

            assert!(has_image(
                &actor.chat_state_handle.get_conversation().await,
                IMAGE
            ));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn image_strip_server_rejection_waits_for_matching_completed() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel();
            let actor =
                Arc::new(create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await);
            seed(&actor, IMAGE).await;
            let request_id = RequestId::from("server-rejected");

            actor
                .handle_sampling_event(stripped(&request_id, &[IMAGE], StripReason::ServerRejected))
                .await;
            assert!(has_image(
                &actor.chat_state_handle.get_conversation().await,
                IMAGE
            ));

            actor
                .handle_sampling_event(completed(&RequestId::from("unrelated")))
                .await;
            settle().await;
            assert!(has_image(
                &actor.chat_state_handle.get_conversation().await,
                IMAGE
            ));

            actor.handle_sampling_event(completed(&request_id)).await;
            for _ in 0..100 {
                if !has_image(&actor.chat_state_handle.get_conversation().await, IMAGE) {
                    return;
                }
                tokio::task::yield_now().await;
            }
            panic!("matching completion did not apply the pending strip");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn image_strip_failed_clears_pending_state() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel();
            let actor =
                Arc::new(create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await);
            seed(&actor, IMAGE).await;
            let request_id = RequestId::from("failed");

            actor
                .handle_sampling_event(stripped(&request_id, &[IMAGE], StripReason::ServerRejected))
                .await;
            actor.handle_sampling_event(failed(&request_id)).await;
            actor.handle_sampling_event(completed(&request_id)).await;
            settle().await;

            assert!(has_image(
                &actor.chat_state_handle.get_conversation().await,
                IMAGE
            ));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn image_strip_requires_one_unique_url() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel();
            let actor =
                Arc::new(create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await);
            let second = "data:image/png;base64,SECOND";
            seed(&actor, IMAGE).await;
            seed(&actor, second).await;

            let ambiguous = RequestId::from("ambiguous");
            actor
                .handle_sampling_event(stripped(
                    &ambiguous,
                    &[IMAGE, second],
                    StripReason::ServerRejected,
                ))
                .await;
            actor.handle_sampling_event(completed(&ambiguous)).await;
            settle().await;
            let conversation = actor.chat_state_handle.get_conversation().await;
            assert!(has_image(&conversation, IMAGE) && has_image(&conversation, second));

            seed(&actor, IMAGE).await;
            let duplicate = RequestId::from("duplicate");
            actor
                .handle_sampling_event(stripped(
                    &duplicate,
                    &[IMAGE, IMAGE],
                    StripReason::ServerRejected,
                ))
                .await;
            actor.handle_sampling_event(completed(&duplicate)).await;
            for _ in 0..100 {
                let conversation = actor.chat_state_handle.get_conversation().await;
                if !has_image(&conversation, IMAGE) {
                    assert!(has_image(&conversation, second));
                    return;
                }
                tokio::task::yield_now().await;
            }
            panic!("one unique URL did not persist all matching occurrences");
        })
        .await;
}
