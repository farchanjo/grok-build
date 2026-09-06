//! Sampler-specific helpers for the shared-HTTP-client integration binaries:
//! config + request drivers for real `InferenceClient`s. The generic
//! connection-counting server lives in `xai_grok_test_support`.

use std::sync::Arc;

use xai_grok_inference::{InferenceClient, InferenceConfig};
use xai_grok_inference_types::{ContentPart, ConversationItem, ConversationRequest, UserItem};

pub fn test_config(base_url: &str, api_key: &str) -> InferenceConfig {
    InferenceConfig {
        api_key: Some(api_key.to_string()),
        base_url: base_url.to_string(),
        model: "test-model".to_string(),
        ..InferenceConfig::default()
    }
}

/// Canned conversation request for wire-level tests; the body is not a valid
/// completion, but only the wire-level request matters here.
pub fn conversation_request() -> ConversationRequest {
    ConversationRequest {
        items: vec![ConversationItem::User(UserItem {
            content: vec![ContentPart::Text {
                text: Arc::<str>::from("hi"),
            }],
            ..Default::default()
        })],
        ..Default::default()
    }
}

/// Drive one POST through the client; the canned `{}` body is not a valid
/// completion, but only the wire-level request matters here.
pub async fn send_one(client: &InferenceClient) {
    let _ = client.conversation(conversation_request()).await;
}
