use super::support::*;
use super::*;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::post;
use axum::{Json, Router};
use futures_util::stream;
use serde_json::{Value, json};
use std::convert::Infallible;
use xai_grok_tools::types::output::{
    FileContent, ImageContent as ToolImageContent, ReadFileOutput, ToolOutput, ToolRunResult,
};

async fn describe_tool_image(
    State(requests): State<tokio::sync::mpsc::UnboundedSender<Value>>,
    Json(body): Json<Value>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let _ = requests.send(body);
    let events = vec![
        Event::default().data(
            json!({
                "id": "chatcmpl-image-route",
                "object": "chat.completion.chunk",
                "created": 1_234_567_890,
                "model": "vision-model",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "role": "assistant",
                        "content": "A brown square returned by the read_file tool."
                    },
                    "finish_reason": "stop"
                }]
            })
            .to_string(),
        ),
        Event::default().data("[DONE]"),
    ];
    Sse::new(stream::iter(events.into_iter().map(Ok::<_, Infallible>)))
        .keep_alive(KeepAlive::default())
}

#[tokio::test(flavor = "current_thread")]
async fn read_file_image_uses_vision_route_when_active_model_cannot_see_images() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (request_tx, mut request_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
            let app = Router::new()
                .route("/v1/chat/completions", post(describe_tool_image))
                .with_state(request_tx);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind image-describe server");
            let addr = listener.local_addr().expect("image-describe server address");
            let server = tokio::spawn(async move {
                axum::serve(listener, app)
                    .await
                    .expect("serve image-describe response");
            });

            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let descriptor_dir = tempfile::tempdir().expect("image descriptor directory");
            let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.media_descriptor_store = std::sync::Arc::new(
                crate::session::media_descriptors::MediaDescriptorStore::empty(
                    descriptor_dir.path(),
                ),
            );

            let mut active_settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .expect("active inference settings");
            active_settings.supports_image_input = Some(false);
            actor
                .chat_state_handle
                .update_inference_settings(active_settings);
            assert_eq!(
                actor
                    .chat_state_handle
                    .get_inference_settings()
                    .await
                    .and_then(|settings| settings.supports_image_input),
                Some(false),
                "the active coding model must explicitly disclaim image input",
            );

            let mut vision_entry = crate::agent::config::ModelEntry {
                info: crate::agent::config::ModelInfo::fallback("vision-model"),
                model_provider: None,
                api_key: Some("vision-test-key".to_owned()),
                env_key: None,
                auth_provider: None,
                api_base_url: None,
            };
            vision_entry.info.id = Some("vision-route".to_owned());
            vision_entry.info.base_url = format!("http://{addr}/v1");
            vision_entry.info.api_backend = crate::inference::ApiBackend::ChatCompletions;
            vision_entry.info.supports_image_input = Some(true);
            actor
                .models_manager
                .insert_test_entry("vision-route", vision_entry);
            actor.media_config.borrow_mut().image_model = Some("vision-route".to_owned());

            let image = test_image_content();
            let image_data = image.data.clone();
            let result = ToolRunResult {
                output: ToolOutput::ReadFile(ReadFileOutput::ImageContent(ToolImageContent {
                    data: image.data,
                    mime_type: image.mime_type,
                    annotations: None,
                    uri: None,
                    meta: None,
                })),
                prompt_text: "raw read_file image result".to_owned(),
                effective_tool_name: None,
            };
            let followups = actor
                .handle_bridge_tool_success(
                    &acp::ToolCallId::new("call-image-route"),
                    "call-image-route",
                    "read_file",
                    "read_file",
                    result,
                    0,
                    "test",
                    &json!({"target_file": "/tmp/non-vision-model.png"}),
                )
                .await
                .expect("tool image routing should succeed");
            assert!(
                followups.is_empty(),
                "typed read_file images should not become deferred inline-image messages",
            );

            let request = tokio::time::timeout(std::time::Duration::from_secs(1), request_rx.recv())
                .await
                .expect("vision route should receive a request")
                .expect("vision route request channel should stay open");
            assert_eq!(request.get("model").and_then(Value::as_str), Some("vision-model"));
            let request_json = request.to_string();
            assert!(
                request_json.contains("data:image/png;base64,"),
                "the configured vision route must receive the tool image: {request_json}",
            );
            assert!(
                request_json.contains(&image_data),
                "the vision route must receive the original image payload",
            );

            let conversation = actor.chat_state_handle.get_conversation().await;
            let tool_result = match conversation.last() {
                Some(ConversationItem::ToolResult(tool_result)) => tool_result,
                other => panic!("conversation must end with a tool result, got {other:?}"),
            };
            assert_eq!(tool_result.tool_call_id, "call-image-route");
            assert!(
                tool_result.images.is_empty(),
                "a non-vision coding model must not receive an inline image",
            );
            assert!(
                tool_result.content.contains("Read image file: /tmp/non-vision-model.png")
                    && tool_result
                        .content
                        .contains("<image_description>\nA brown square returned by the read_file tool.\n</image_description>"),
                "the coding model should receive the routed text description: {}",
                tool_result.content,
            );
            assert!(
                !tool_result.content.contains("data:image/")
                    && !tool_result.content.contains(&image_data),
                "raw image data must not reach the non-vision coding model",
            );

            server.abort();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn read_file_image_auto_routes_session_pin_to_catalog_vision() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (request_tx, mut request_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
            let app = Router::new()
                .route("/v1/chat/completions", post(describe_tool_image))
                .with_state(request_tx);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind image-describe server");
            let addr = listener.local_addr().expect("image-describe server address");
            let server = tokio::spawn(async move {
                axum::serve(listener, app)
                    .await
                    .expect("serve image-describe response");
            });

            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let descriptor_dir = tempfile::tempdir().expect("image descriptor directory");
            let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.media_descriptor_store = std::sync::Arc::new(
                crate::session::media_descriptors::MediaDescriptorStore::empty(
                    descriptor_dir.path(),
                ),
            );

            let mut active_settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .expect("active inference settings");
            active_settings.supports_image_input = Some(false);
            actor
                .chat_state_handle
                .update_inference_settings(active_settings);

            let mut vision_entry = crate::agent::config::ModelEntry {
                info: crate::agent::config::ModelInfo::fallback("vision-model"),
                model_provider: None,
                api_key: Some("vision-test-key".to_owned()),
                env_key: None,
                auth_provider: None,
                api_base_url: None,
            };
            vision_entry.info.id = Some("vision-route".to_owned());
            vision_entry.info.base_url = format!("http://{addr}/v1");
            vision_entry.info.api_backend = crate::inference::ApiBackend::ChatCompletions;
            vision_entry.info.supports_image_input = Some(true);
            actor
                .models_manager
                .insert_test_entry("vision-route", vision_entry);
            actor.media_config.borrow_mut().image_model = Some("@session".to_owned());

            let image = test_image_content();
            let image_data = image.data.clone();
            let result = ToolRunResult {
                output: ToolOutput::ReadFile(ReadFileOutput::ImageContent(ToolImageContent {
                    data: image.data,
                    mime_type: image.mime_type,
                    annotations: None,
                    uri: None,
                    meta: None,
                })),
                prompt_text: "raw read_file image result".to_owned(),
                effective_tool_name: None,
            };
            let followups = actor
                .handle_bridge_tool_success(
                    &acp::ToolCallId::new("call-image-auto-route"),
                    "call-image-auto-route",
                    "read_file",
                    "read_file",
                    result,
                    0,
                    "test",
                    &json!({"target_file": "/tmp/auto-route.png"}),
                )
                .await
                .expect("tool image auto-route should succeed");
            assert!(followups.is_empty());

            let request = tokio::time::timeout(std::time::Duration::from_secs(1), request_rx.recv())
                .await
                .expect("catalog vision fallback should receive a request")
                .expect("vision route request channel should stay open");
            assert_eq!(request.get("model").and_then(Value::as_str), Some("vision-model"));
            let request_json = request.to_string();
            assert!(
                request_json.contains("data:image/png;base64,"),
                "the auto-selected vision route must receive the tool image: {request_json}",
            );
            assert!(
                request_json.contains(&image_data),
                "the vision route must receive the original image payload",
            );

            let conversation = actor.chat_state_handle.get_conversation().await;
            let tool_result = match conversation.last() {
                Some(ConversationItem::ToolResult(tool_result)) => tool_result,
                other => panic!("conversation must end with a tool result, got {other:?}"),
            };
            assert!(
                tool_result.images.is_empty(),
                "a non-vision coding model must not receive an inline image",
            );
            assert!(
                tool_result.content.contains("Read image file: /tmp/auto-route.png")
                    && tool_result
                        .content
                        .contains("<image_description>"),
                "the coding model should receive the routed text description: {}",
                tool_result.content,
            );

            server.abort();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn read_file_source_text_stays_on_session_and_does_not_hit_file_model() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (request_tx, mut request_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
            let app = Router::new()
                .route("/v1/chat/completions", post(describe_tool_image))
                .with_state(request_tx);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind unused file-model server");
            let addr = listener.local_addr().expect("file-model server address");
            let server = tokio::spawn(async move {
                axum::serve(listener, app)
                    .await
                    .expect("serve unused file-model response");
            });

            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;

            let mut active_settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .expect("active inference settings");
            active_settings.supports_image_input = Some(false);
            actor
                .chat_state_handle
                .update_inference_settings(active_settings);

            let mut file_entry = crate::agent::config::ModelEntry {
                info: crate::agent::config::ModelInfo::fallback("vision-model"),
                model_provider: None,
                api_key: Some("file-route-test-key".to_owned()),
                env_key: None,
                auth_provider: None,
                api_base_url: None,
            };
            file_entry.info.id = Some("file-route".to_owned());
            file_entry.info.base_url = format!("http://{addr}/v1");
            file_entry.info.api_backend = crate::inference::ApiBackend::ChatCompletions;
            file_entry.info.supports_image_input = Some(true);
            actor
                .models_manager
                .insert_test_entry("file-route", file_entry);
            actor.media_config.borrow_mut().file_model = Some("file-route".to_owned());
            actor.media_config.borrow_mut().image_model = Some("file-route".to_owned());

            let source = "     1|fn main() {\n     2|    println!(\"hi\");\n     3|}\n";
            let result = ToolRunResult {
                output: ToolOutput::ReadFile(ReadFileOutput::FileContent(FileContent {
                    content: source.to_owned(),
                    content_concise: None,
                    absolute_path: std::path::PathBuf::from("/tmp/main.rs"),
                    offset: None,
                    limit: None,
                    raw_output: source.to_owned(),
                    total_lines: 3,
                    extracted_images: Vec::new(),
                })),
                prompt_text: source.to_owned(),
                effective_tool_name: None,
            };
            let followups = actor
                .handle_bridge_tool_success(
                    &acp::ToolCallId::new("call-source-text"),
                    "call-source-text",
                    "read_file",
                    "read_file",
                    result,
                    0,
                    "test",
                    &json!({"target_file": "/tmp/main.rs"}),
                )
                .await
                .expect("source read_file should succeed without an auxiliary model");
            assert!(
                followups.is_empty(),
                "source text must not spawn image follow-ups: {followups:?}"
            );

            let conversation = actor.chat_state_handle.get_conversation().await;
            let tool_result = match conversation.last() {
                Some(ConversationItem::ToolResult(tool_result)) => tool_result,
                other => panic!("conversation must end with a tool result, got {other:?}"),
            };
            assert_eq!(
                tool_result.content.as_ref(),
                source,
                "source text must reach the session model unchanged"
            );
            assert!(
                !tool_result.content.contains("<image_description>")
                    && !tool_result.content.contains("<pdf_description>"),
                "source text must not be rewritten by the file/image model"
            );
            assert!(
                tool_result.images.is_empty(),
                "source text must not be inlined as images"
            );

            let missed = tokio::time::timeout(
                std::time::Duration::from_millis(200),
                request_rx.recv(),
            )
            .await;
            assert!(
                missed.is_err(),
                "the file/image model must not be called for source text: {missed:?}"
            );

            server.abort();
        })
        .await;
}
