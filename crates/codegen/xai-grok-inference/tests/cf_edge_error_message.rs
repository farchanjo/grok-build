//! Wire e2e + pure tests for user-facing API error sanitization.
//!
//! Edge proxies return non-JSON bodies (HTML). Those must never reach TUI
//! scrollback; only structured JSON error envelopes and status-based copy.

use std::sync::Arc;

use xai_grok_inference::config::ProviderIdentity;
use xai_grok_inference::{InferenceConfig, InferenceClient};
use xai_grok_inference_types::{
    ContentPart, ConversationItem, ConversationRequest, UserItem, status_user_message,
    status_user_message_for, user_facing_api_error_message,
};
use xai_grok_test_support::{MockInferenceServer, ScriptedResponse};

const CF_524_HTML: &str = r#"<!DOCTYPE html>
<html lang="en-US">
<head><title>grok.com | 524: A timeout occurred</title></head>
<body>
  <h1>A timeout occurred <span>Error code 524</span></h1>
  <div>Visit cloudflare.com for more information.</div>
</body>
</html>"#;

fn test_config_for(base_url: &str, api_key: &str, provider: ProviderIdentity) -> InferenceConfig {
    InferenceConfig {
        api_key: Some(api_key.to_string()),
        base_url: base_url.to_string(),
        model: "test-model".to_string(),
        provider_identity: provider,
        ..InferenceConfig::default()
    }
}

fn user_request(text: &str) -> ConversationRequest {
    ConversationRequest {
        items: vec![ConversationItem::User(UserItem {
            content: vec![ContentPart::Text {
                text: Arc::<str>::from(text),
            }],
            ..Default::default()
        })],
        ..Default::default()
    }
}

async fn stream_err(status: u16, body: &str) -> xai_grok_inference_types::InferenceError {
    stream_err_for(status, body, ProviderIdentity::default()).await
}

async fn stream_err_for(
    status: u16,
    body: &str,
    provider: ProviderIdentity,
) -> xai_grok_inference_types::InferenceError {
    let server = MockInferenceServer::start().await.expect("start mock");
    server.enqueue_response("/v1/chat/completions", ScriptedResponse::text(status, body));
    let mut cfg = test_config_for(&server.url(), "test-key", provider);
    cfg.max_retries = Some(0);
    let client = InferenceClient::new(cfg).expect("client");
    match client.conversation_stream(user_request("hi")).await {
        Ok(_) => panic!("expected API error"),
        Err(e) => e,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_524_html_uses_status_copy() {
    let err = stream_err(524, CF_524_HTML).await;
    let s = err.to_string();
    assert!(!s.contains("<!DOCTYPE") && !s.contains("<html"));
    // The default test config targets the Custom provider, so the status copy
    // uses the neutral "the model provider" label, not "Grok".
    let label = ProviderIdentity::Custom.label();
    assert!(s.contains(&status_user_message_for(
        reqwest::StatusCode::from_u16(524).unwrap(),
        label
    )));
    assert!(s.contains("524"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_503_html_uses_unavailable_copy() {
    let err = stream_err(503, "<html><body>Service Unavailable</body></html>").await;
    let s = err.to_string();
    assert!(!s.contains("<html"));
    assert!(s.contains("temporarily unavailable"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_json_error_envelope_is_preserved() {
    let body = r#"{"error":{"message":"rate limit exceeded","type":"rate_limit_error"}}"#;
    let err = stream_err(429, body).await;
    let s = err.to_string();
    assert!(s.contains("rate limit exceeded"));
    assert!(!s.contains("temporarily unavailable"));
}

#[test]
fn status_user_message_matrix() {
    let cases: &[(u16, &str)] = &[
        (502, "temporarily unavailable"),
        (503, "temporarily unavailable"),
        (504, "temporarily unavailable"),
        (520, "timed out"),
        (524, "timed out"),
        (500, "Something went wrong on the server"),
        (400, "Request failed"),
    ];
    for &(code, needle) in cases {
        let msg = status_user_message(reqwest::StatusCode::from_u16(code).unwrap());
        assert!(
            msg.contains(needle),
            "status {code}: expected {needle:?} in {msg:?}"
        );
        assert!(
            msg.contains(&format!("HTTP {code}")),
            "status {code}: expected HTTP code in {msg:?}"
        );
    }
}

#[test]
fn non_json_empty_body_falls_back_to_status() {
    let msg = user_facing_api_error_message(reqwest::StatusCode::BAD_GATEWAY, b"");
    assert_eq!(msg, status_user_message(reqwest::StatusCode::BAD_GATEWAY));
}

#[test]
fn structured_json_is_not_replaced_by_status_copy() {
    let bytes = br#"{"error":{"message":"credits exhausted","type":"server_error"}}"#;
    let msg = user_facing_api_error_message(reqwest::StatusCode::PAYMENT_REQUIRED, bytes);
    assert_eq!(msg, "credits exhausted");
}

/// 502/520-class errors must name the actual provider (never "Grok" for
/// non-xAI providers).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_502_openrouter_names_provider_not_grok() {
    let err = stream_err_for(
        502,
        "<html>bad gateway</html>",
        ProviderIdentity::OpenRouter,
    )
    .await;
    let s = err.to_string();
    assert!(s.contains("OpenRouter"));
    assert!(!s.contains("Grok"));
    assert!(s.contains("temporarily unavailable"));
    assert!(s.contains("502"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_520_openai_names_provider_not_grok() {
    let err = stream_err_for(520, "<html>edge timeout</html>", ProviderIdentity::OpenAi).await;
    let s = err.to_string();
    assert!(s.contains("OpenAI"));
    assert!(!s.contains("Grok"));
    assert!(s.contains("timed out"));
    assert!(s.contains("520"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_502_xai_keeps_grok_wording() {
    let err = stream_err_for(502, "<html>bad gateway</html>", ProviderIdentity::Xai).await;
    let s = err.to_string();
    assert!(s.contains("Grok"));
    assert!(s.contains("temporarily unavailable"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_502_custom_uses_neutral_label() {
    let err = stream_err_for(502, "<html>bad gateway</html>", ProviderIdentity::Custom).await;
    let s = err.to_string();
    assert!(s.contains("the model provider"));
    assert!(!s.contains("Grok"));
    assert!(!s.contains("OpenRouter"));
}

/// HTTP 402 (OpenRouter out-of-credits) produces a dedicated, actionable
/// credits message naming the provider, and is never retried.
#[test]
fn status_402_openrouter_dedicated_credits_message() {
    let msg = status_user_message_for(reqwest::StatusCode::PAYMENT_REQUIRED, "OpenRouter");
    assert!(msg.contains("OpenRouter account out of credits"));
    assert!(msg.contains("add credits"));
    assert!(msg.contains("HTTP 402"));
}

#[test]
fn status_402_never_says_grok_for_openrouter() {
    let msg = status_user_message_for(reqwest::StatusCode::PAYMENT_REQUIRED, "OpenRouter");
    assert!(!msg.contains("Grok"));
}

/// Non-JSON 402 surfaces the dedicated credits copy; a structured envelope
/// still wins (existing contract).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_402_non_json_openrouter_dedicated_message() {
    let err = stream_err_for(
        402,
        "<html>Payment Required</html>",
        ProviderIdentity::OpenRouter,
    )
    .await;
    let s = err.to_string();
    assert!(s.contains("OpenRouter account out of credits"));
    assert!(s.contains("add credits"));
    assert!(s.contains("402"));
}

/// When the diagnostics body carries the selected upstream's
/// `provider_name` (but no parseable error envelope), the 502 status
/// copy names that specific upstream — proving `provider_name` precedence
/// over the generic provider label.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_502_prefers_diagnostics_upstream_name() {
    // No `error` envelope → structured_error_message returns None → status
    // copy fallback. The body still carries OpenRouter routing metadata
    // naming the selected upstream, so diagnostics.provider_name resolves.
    let body = r#"{
        "openrouter_metadata": {
            "attempts": [{"provider": "DeepInfra", "status": 502}]
        }
    }"#;
    let err = stream_err_for(502, body, ProviderIdentity::OpenRouter).await;
    let s = err.to_string();
    assert!(
        s.contains("DeepInfra"),
        "expected the selected upstream name in the 502 copy: {s}"
    );
    assert!(s.contains("temporarily unavailable"));
    assert!(s.contains("502"));
}
