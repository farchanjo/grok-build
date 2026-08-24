use super::support::*;
use super::*;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::post;
use axum::{Json, Router};
use futures_util::stream;
use serde_json::{Value, json};
use std::convert::Infallible;
use std::sync::Mutex;
use xai_grok_tools::types::output::VideoContent;
use xai_grok_tools::util::ffmpeg::{FfmpegError, ProcessOutput, ProcessRunner};

struct VideoFrameRunner {
    frame: Vec<u8>,
    calls: Mutex<Vec<String>>,
}

impl ProcessRunner for VideoFrameRunner {
    fn run(
        &self,
        program: &str,
        _args: &[&str],
        _timeout: std::time::Duration,
    ) -> Result<ProcessOutput, FfmpegError> {
        self.calls
            .lock()
            .expect("video runner calls lock")
            .push(program.to_owned());
        match program {
            "ffprobe" => Ok(ProcessOutput {
                stdout: br#"{"format":{"duration":"1.0"},"streams":[{"codec_type":"video","codec_name":"h264","width":640,"height":360}]}"#
                    .to_vec(),
                stderr: Vec::new(),
                status_code: 0,
            }),
            "ffmpeg" => Ok(ProcessOutput {
                stdout: self.frame.clone(),
                stderr: Vec::new(),
                status_code: 0,
            }),
            other => panic!("unexpected media helper {other}"),
        }
    }
}

async fn describe_video_frame(
    State(requests): State<tokio::sync::mpsc::UnboundedSender<Value>>,
    Json(body): Json<Value>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let _ = requests.send(body);
    let events = vec![
        Event::default().data(
            json!({
                "id": "chatcmpl-video-route",
                "object": "chat.completion.chunk",
                "created": 1_234_567_890,
                "model": "video-vision-model",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "role": "assistant",
                        "content": "The sampled frame shows a terminal with a successful build."
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
async fn video_model_overrides_image_route_and_describes_sampled_frames() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (request_tx, mut request_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
            let app = Router::new()
                .route("/v1/chat/completions", post(describe_video_frame))
                .with_state(request_tx);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind video-frame describe server");
            let addr = listener
                .local_addr()
                .expect("video-frame describe server address");
            let server = tokio::spawn(async move {
                axum::serve(listener, app)
                    .await
                    .expect("serve video-frame description");
            });

            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let descriptor_dir = tempfile::tempdir().expect("video descriptor directory");
            let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.media_descriptor_store = std::sync::Arc::new(
                crate::session::media_descriptors::MediaDescriptorStore::empty(
                    descriptor_dir.path(),
                ),
            );

            let mut video_entry = crate::agent::config::ModelEntry {
                info: crate::agent::config::ModelInfo::fallback("video-vision-model"),
                model_provider: None,
                api_key: Some("video-route-test-key".to_owned()),
                env_key: None,
                auth_provider: None,
                api_base_url: None,
            };
            video_entry.info.id = Some("video-route".to_owned());
            video_entry.info.base_url = format!("http://{addr}/v1");
            video_entry.info.api_backend = crate::inference::ApiBackend::ChatCompletions;
            video_entry.info.supports_image_input = Some(true);
            actor
                .models_manager
                .insert_test_entry("video-route", video_entry);

            {
                let mut media = actor.media_config.borrow_mut();
                media.image_model = Some("unused-image-route".to_owned());
                media.video_model = Some("video-route".to_owned());
                media.video_max_frames = 1;
            }

            let dir = tempfile::tempdir().expect("video fixture directory");
            let video_path = dir.path().join("routing-check.mp4");
            std::fs::write(&video_path, b"video fixture").expect("write video fixture");
            let frame = b"sampled-jpeg-frame".to_vec();
            let runner = VideoFrameRunner {
                frame: frame.clone(),
                calls: Mutex::new(Vec::new()),
            };
            let video = VideoContent {
                absolute_path: video_path,
                mime_type: "video/mp4".to_owned(),
                size_bytes: 13,
                duration_secs: Some(1.0),
                width: Some(640),
                height: Some(360),
                has_audio: false,
            };
            let media = actor.media_config.borrow().clone();
            let text = actor
                .understand_tool_video(&video, &media, &runner, None)
                .await
                .expect("video route should describe sampled frames");

            let request =
                tokio::time::timeout(std::time::Duration::from_secs(1), request_rx.recv())
                    .await
                    .expect("video route should receive a request")
                    .expect("video route request channel should stay open");
            assert_eq!(
                request.get("model").and_then(Value::as_str),
                Some("video-vision-model"),
                "the explicit video route must override the image route",
            );
            let request_json = request.to_string();
            let frame_base64 =
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &frame);
            assert!(
                request_json.contains("data:image/jpeg;base64,")
                    && request_json.contains(&frame_base64),
                "the video route must receive the extracted JPEG frame: {request_json}",
            );
            assert!(
                request_json
                    .contains("Describe this image for a coding assistant that cannot see it."),
                "tool-video frames must use the stable media-description prompt",
            );

            assert!(
                text.contains("<video_description>")
                    && text.contains("Sampled 1 frame(s):")
                    && text.contains(
                        "Frame 1:\nThe sampled frame shows a terminal with a successful build."
                    ),
                "video understanding should return the sampled-frame description: {text}",
            );
            assert!(
                !text.contains("data:image/") && !text.contains(&frame_base64),
                "raw frame data must not enter the coding model's video descriptor",
            );
            assert_eq!(
                runner
                    .calls
                    .lock()
                    .expect("video runner calls lock")
                    .as_slice(),
                ["ffprobe", "ffmpeg"],
                "video understanding should probe once and sample one configured frame",
            );

            let descriptor = actor
                .media_descriptor_store
                .snapshot()
                .values()
                .find(|descriptor| {
                    descriptor.key.modality
                        == crate::session::media_descriptors::MediaModality::Video
                })
                .cloned()
                .expect("video descriptor should be persisted");
            assert_eq!(descriptor.model_id.as_deref(), Some("video-vision-model"));
            assert_eq!(descriptor.provider.as_deref(), Some("Grok"));
            assert!(
                descriptor
                    .description
                    .contains("The sampled frame shows a terminal with a successful build."),
            );

            server.abort();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn missing_video_route_stays_actionable_instead_of_collapsing_to_disabled() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
            actor.media_config.borrow_mut().video_model = Some("missing-video-route".to_owned());

            let dir = tempfile::tempdir().expect("video fixture directory");
            let video_path = dir.path().join("unusable-route.mp4");
            std::fs::write(&video_path, b"video fixture").expect("write video fixture");
            let runner = VideoFrameRunner {
                frame: b"sampled-jpeg-frame".to_vec(),
                calls: Mutex::new(Vec::new()),
            };
            let video = VideoContent {
                absolute_path: video_path,
                mime_type: "video/mp4".to_owned(),
                size_bytes: 13,
                duration_secs: Some(1.0),
                width: Some(640),
                height: Some(360),
                has_audio: false,
            };
            let media = actor.media_config.borrow().clone();
            let text = actor
                .understand_tool_video(&video, &media, &runner, None)
                .await
                .expect("unusable video route must still emit a descriptor");

            assert!(
                text.contains("auxiliary model `missing-video-route` is not in the catalog")
                    || text.contains("missing-video-route"),
                "video route failures must stay actionable: {text}"
            );
            assert!(
                !text.contains("media understanding is disabled"),
                "video route failures must not collapse to Disabled: {text}"
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_video_describe_client_fails_closed_like_image_pdf() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;

            let mut video_entry = crate::agent::config::ModelEntry {
                info: crate::agent::config::ModelInfo::fallback("video-vision-model"),
                model_provider: None,
                api_key: Some("bad\nkey".to_owned()),
                env_key: None,
                auth_provider: None,
                api_base_url: None,
            };
            video_entry.info.id = Some("video-bad-client".to_owned());
            video_entry.info.base_url = "http://127.0.0.1:9/v1".to_owned();
            video_entry.info.api_backend = crate::inference::ApiBackend::ChatCompletions;
            video_entry.info.supports_image_input = Some(true);
            actor
                .models_manager
                .insert_test_entry("video-bad-client", video_entry);
            actor.media_config.borrow_mut().video_model = Some("video-bad-client".to_owned());

            let dir = tempfile::tempdir().expect("video fixture directory");
            let video_path = dir.path().join("bad-client.mp4");
            std::fs::write(&video_path, b"video fixture").expect("write video fixture");
            let runner = VideoFrameRunner {
                frame: b"sampled-jpeg-frame".to_vec(),
                calls: Mutex::new(Vec::new()),
            };
            let video = VideoContent {
                absolute_path: video_path,
                mime_type: "video/mp4".to_owned(),
                size_bytes: 13,
                duration_secs: Some(1.0),
                width: Some(640),
                height: Some(360),
                has_audio: false,
            };
            let media = actor.media_config.borrow().clone();
            let err = actor
                .understand_tool_video(&video, &media, &runner, None)
                .await
                .expect_err("invalid sampling client must fail closed");
            let err_text = err.to_string();
            assert!(
                err_text.contains("failed to build video-describe sampling client"),
                "client construction must match image/PDF fail-closed semantics: {err_text}"
            );
            assert!(
                !err_text.contains("bad\nkey") && !err_text.contains("Bearer "),
                "client construction errors must not leak secrets: {err_text}"
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn video_session_pin_auto_routes_to_catalog_vision() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (request_tx, mut request_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
            let app = Router::new()
                .route("/v1/chat/completions", post(describe_video_frame))
                .with_state(request_tx);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind video-frame describe server");
            let addr = listener
                .local_addr()
                .expect("video-frame describe server address");
            let server = tokio::spawn(async move {
                axum::serve(listener, app)
                    .await
                    .expect("serve video-frame description");
            });

            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let descriptor_dir = tempfile::tempdir().expect("video descriptor directory");
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
                info: crate::agent::config::ModelInfo::fallback("video-vision-model"),
                model_provider: None,
                api_key: Some("video-route-test-key".to_owned()),
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
            {
                let mut media = actor.media_config.borrow_mut();
                media.image_model = Some("@session".to_owned());
                media.video_model = None;
                media.video_max_frames = 1;
            }

            let dir = tempfile::tempdir().expect("video fixture directory");
            let video_path = dir.path().join("auto-route.mp4");
            std::fs::write(&video_path, b"video fixture").expect("write video fixture");
            let frame = b"sampled-jpeg-frame".to_vec();
            let runner = VideoFrameRunner {
                frame: frame.clone(),
                calls: Mutex::new(Vec::new()),
            };
            let video = VideoContent {
                absolute_path: video_path,
                mime_type: "video/mp4".to_owned(),
                size_bytes: 13,
                duration_secs: Some(1.0),
                width: Some(640),
                height: Some(360),
                has_audio: false,
            };
            let media = actor.media_config.borrow().clone();
            let text = actor
                .understand_tool_video(&video, &media, &runner, None)
                .await
                .expect("video @session pin should auto-route to catalog vision");

            let request =
                tokio::time::timeout(std::time::Duration::from_secs(1), request_rx.recv())
                    .await
                    .expect("catalog vision fallback should receive a frame describe request")
                    .expect("vision route request channel should stay open");
            assert_eq!(
                request.get("model").and_then(Value::as_str),
                Some("video-vision-model"),
            );
            assert!(
                text.contains("The sampled frame shows a terminal with a successful build.")
                    || text.contains("sampled"),
                "frame description should reach the coding model: {text}"
            );
            server.abort();
        })
        .await;
}
