//! Shell-owned media-understanding orchestration shared by user attachments,
//! `read_file` tool results, and compaction backfill.
//!
//! Tools never hold an inference client. This module converts typed image /
//! audio / video payloads into scrubbed text descriptors. Raw audio/video
//! content variants are never persisted into conversation history.

use std::io::{self, Read};
use std::path::Path;

use xai_grok_tools::types::output::{AudioContent, VideoContent};
use xai_grok_tools::util::ffmpeg::{
    self, DEFAULT_EXTRACT_TIMEOUT, DEFAULT_PROBE_TIMEOUT, MAX_MEDIA_INPUT_BYTES, ProcessRunner,
    SystemProcessRunner,
};

use crate::config::MediaConfig;
use crate::session::image_describe::{
    DescribeError, ImageDescribeCache, ImageDescribeSource, content_fingerprint,
    render_image_description_block, stable_describe_prompt_fingerprint,
};
use crate::session::media_descriptors::{
    MediaDescriptor, MediaDescriptorKey, MediaDescriptorSource, MediaDescriptorStore, MediaModality,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaFailurePolicy {
    AbortTurn,
    Placeholder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MediaPolicyError {
    #[error("media understanding is disabled ([media].mode = off)")]
    Disabled,
    #[error(
        "user media is not automatically understood when [media].mode = tools_only (use read_file on the saved asset)"
    )]
    UserMediaSkippedByMode,
    #[error("ZDR policy blocks auxiliary media disclosure to {provider}")]
    ExternalProviderBlockedByZdr { provider: &'static str },
}

/// Whether auxiliary media inference may run for this source. Native model
/// input is handled before this gate and remains available in every mode.
pub fn auxiliary_media_allowed(
    mode: crate::config::MediaMode,
    source: ImageDescribeSource,
) -> Result<(), MediaPolicyError> {
    match mode {
        crate::config::MediaMode::Auto => Ok(()),
        crate::config::MediaMode::ToolsOnly => match source {
            ImageDescribeSource::ToolRead => Ok(()),
            ImageDescribeSource::UserAttachment | ImageDescribeSource::CompactionBackfill => {
                Err(MediaPolicyError::UserMediaSkippedByMode)
            }
        },
        crate::config::MediaMode::Off => Err(MediaPolicyError::Disabled),
    }
}

/// Fail closed when a ZDR team's media route targets a provider outside xAI.
///
/// Provider identity comes from the resolved inference configuration, while
/// the ZDR verdict comes only from server-issued account metadata. Local TOML,
/// permission mode, and tool arguments cannot override this gate.
pub fn auxiliary_media_provider_allowed(
    provider: xai_grok_inference::config::ProviderIdentity,
    auth: Option<&crate::auth::GrokAuth>,
) -> Result<(), MediaPolicyError> {
    let blocked =
        !provider.is_first_party() && auth.is_some_and(crate::auth::GrokAuth::is_zdr_team);
    if blocked {
        Err(MediaPolicyError::ExternalProviderBlockedByZdr {
            provider: provider.label(),
        })
    } else {
        Ok(())
    }
}

pub fn auxiliary_media_route_allowed(
    provider: xai_grok_inference::config::ProviderIdentity,
    auth_manager: Option<&std::sync::Arc<crate::auth::AuthManager>>,
) -> Result<(), MediaPolicyError> {
    let auth = auth_manager.and_then(|manager| manager.current_or_expired());
    auxiliary_media_provider_allowed(provider, auth.as_ref())
}

fn descriptor_source(source: ImageDescribeSource) -> MediaDescriptorSource {
    match source {
        ImageDescribeSource::UserAttachment => MediaDescriptorSource::UserAttachment,
        ImageDescribeSource::ToolRead => MediaDescriptorSource::ToolRead,
        ImageDescribeSource::CompactionBackfill => MediaDescriptorSource::CompactionBackfill,
    }
}

fn validate_media_metadata(metadata: &std::fs::Metadata) -> io::Result<()> {
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "media path is not a regular file",
        ));
    }
    if metadata.len() > MAX_MEDIA_INPUT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "media file exceeds the {MAX_MEDIA_INPUT_BYTES}-byte input cap ({} bytes)",
                metadata.len()
            ),
        ));
    }
    Ok(())
}

fn fingerprint_media_reader(mut reader: impl Read) -> io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total_read = 0u64;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total_read = total_read.saturating_add(read as u64);
        if total_read > MAX_MEDIA_INPUT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("media file grew beyond the {MAX_MEDIA_INPUT_BYTES}-byte input cap"),
            ));
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn fingerprint_media_file(path: &Path) -> io::Result<String> {
    let metadata = std::fs::metadata(path)?;
    validate_media_metadata(&metadata)?;
    fingerprint_media_reader(std::fs::File::open(path)?)
}

fn fallback_media_fingerprint(path: &Path, size_bytes: u64, mime_type: &str) -> String {
    content_fingerprint(format!("{}:{size_bytes}:{mime_type}", path.display()).as_bytes())
}

#[allow(clippy::too_many_arguments)]
pub async fn describe_image(
    cache: &ImageDescribeCache,
    store: &MediaDescriptorStore,
    client: xai_grok_inference::InferenceClient,
    model: &str,
    provider: Option<&str>,
    raw_bytes: &[u8],
    mime_type: &str,
    outline: Option<&str>,
    current_query: &str,
    source: ImageDescribeSource,
    asset_path: Option<&Path>,
) -> Result<String, DescribeError> {
    let content_fp = content_fingerprint(raw_bytes);
    let prompt_fp = match source {
        ImageDescribeSource::UserAttachment => {
            crate::session::image_describe::describe_prompt_fingerprint(outline, current_query)
        }
        ImageDescribeSource::ToolRead | ImageDescribeSource::CompactionBackfill => {
            stable_describe_prompt_fingerprint()
        }
    };
    let key = MediaDescriptorKey {
        modality: MediaModality::Image,
        content_fingerprint: content_fp,
        source: descriptor_source(source),
        prompt_fingerprint: prompt_fp,
    };
    if let Some(descriptor) = store.get(&key) {
        return Ok(descriptor.description);
    }

    let path_key = asset_path
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let description = cache
        .get_or_describe(
            client,
            model,
            raw_bytes,
            mime_type,
            outline,
            current_query,
            source,
            path_key,
        )
        .await?;
    let relative_asset_path = asset_path.and_then(|path| {
        path.components()
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<std::path::PathBuf>()
            .to_str()
            .map(str::to_owned)
    });
    match MediaDescriptor::new(
        key,
        description.clone(),
        Some(mime_type.to_owned()),
        Some(model.to_owned()),
        provider.map(str::to_owned),
        relative_asset_path,
    ) {
        Ok(descriptor) => {
            if let Err(error) = store.insert(descriptor) {
                // Descriptor durability is supplemental context. A full/corrupt
                // store must not discard an otherwise successful description.
                tracing::warn!(%error, "failed to persist image media descriptor");
            }
        }
        Err(error) => {
            tracing::warn!(%error, "failed to validate image media descriptor");
        }
    }
    Ok(description)
}

pub fn placeholder_for(modality: MediaModality) -> &'static str {
    match modality {
        MediaModality::Image => "[image]",
        MediaModality::Audio => "[audio]",
        MediaModality::Video => "[video]",
    }
}

pub fn descriptor_envelope(description: &str) -> String {
    render_image_description_block(description)
}

use crate::session::media_stt::{
    AsyncAudioTranscriber, AudioSttError, XAI_STREAMING_STT_ROUTE, stt_allowed_for_source,
    stt_sample_rate, validate_audio_stt_route,
};

/// Convert typed audio into model-facing text. Never persists an Audio content
/// variant — only scrubbed text descriptors / transcripts.
pub async fn understand_audio(
    audio: &AudioContent,
    media: &MediaConfig,
    store: &MediaDescriptorStore,
    source: ImageDescribeSource,
    runner: &dyn ProcessRunner,
    transcriber: Option<&dyn AsyncAudioTranscriber>,
) -> String {
    let path = audio.absolute_path.as_path();
    let (content_fp, fingerprint_error) = match fingerprint_media_file(path) {
        Ok(fingerprint) => (fingerprint, None),
        Err(error) => (
            fallback_media_fingerprint(path, audio.size_bytes, &audio.mime_type),
            Some(error),
        ),
    };
    let prompt_fp = stable_describe_prompt_fingerprint();
    let key = MediaDescriptorKey {
        modality: MediaModality::Audio,
        content_fingerprint: content_fp,
        source: descriptor_source(source),
        prompt_fingerprint: prompt_fp,
    };
    if let Some(descriptor) = store.get(&key) {
        return render_audio_description_block(&descriptor.description);
    }

    let probe = ffmpeg::probe_media(path, runner, DEFAULT_PROBE_TIMEOUT).ok();
    let duration = audio
        .duration_secs
        .or_else(|| probe.as_ref().and_then(|p| p.duration_secs));
    let mut sections = Vec::new();
    sections.push(xai_grok_tools::implementations::read_file::audio_prompt_summary(audio));
    if let Some(error) = fingerprint_error.as_ref() {
        sections.push(format!("Media validation warning: {error}"));
    }
    if let Some(d) = duration
        && d > media.audio_max_seconds as f64
    {
        sections.push(format!(
            "Note: duration {d:.1}s exceeds configured audio_max_seconds={}; only the first window is considered.",
            media.audio_max_seconds
        ));
    }

    let mut stt_route_used: Option<String> = None;
    match (
        stt_allowed_for_source(media.mode, source),
        validate_audio_stt_route(media.audio_model.as_deref()),
    ) {
        (Err(error), _) | (_, Err(error)) => {
            sections.push(format!("Transcript: {error}"));
        }
        (Ok(()), Ok(())) => {
            if let Some(error) = fingerprint_error.as_ref() {
                sections.push(format!("Transcript skipped: {error}"));
            } else {
                let max_seconds = media.audio_max_seconds;
                match ffmpeg::extract_audio_pcm_s16le(
                    path,
                    max_seconds,
                    runner,
                    DEFAULT_EXTRACT_TIMEOUT,
                ) {
                    Ok(pcm) => match transcriber {
                        Some(tx) => match tx.transcribe_pcm_s16le(&pcm, stt_sample_rate()).await {
                            Ok(transcript) if !transcript.trim().is_empty() => {
                                sections.push(format!("Transcript:\n{}", transcript.trim()));
                                stt_route_used = Some(XAI_STREAMING_STT_ROUTE.to_owned());
                            }
                            Ok(_) => sections
                                .push(format!("Transcript: {}", AudioSttError::EmptyTranscript)),
                            Err(error) => sections.push(format!("Transcript: {error}")),
                        },
                        None => {
                            sections.push(format!("Transcript: {}", AudioSttError::AuthUnavailable))
                        }
                    },
                    Err(error) => {
                        sections.push(format!("Audio extraction failed: {error}"));
                    }
                }
            }
        }
    }

    let description = sections.join("\n");
    // Only record the truthful STT route when STT actually ran successfully.
    let mut description = description;
    match MediaDescriptor::new(
        key,
        description.clone(),
        Some(audio.mime_type.clone()),
        stt_route_used,
        None,
        path.file_name().and_then(|n| n.to_str()).map(str::to_owned),
    ) {
        Ok(descriptor) => {
            if let Err(error) = store.insert(descriptor) {
                tracing::warn!(%error, "failed to persist audio media descriptor");
                description.push_str(&format!(
                    "\nDescriptor persistence warning: the transcript may be unavailable to later compaction ({error})."
                ));
            }
        }
        Err(error) => {
            tracing::warn!(%error, "failed to validate audio media descriptor");
            description.push_str(&format!(
                "\nDescriptor persistence warning: the transcript could not be recorded ({error})."
            ));
        }
    }
    render_audio_description_block(&description)
}

/// Convert typed video into model-facing text via sampled frame descriptions.
///
/// Frame extraction reuses the process-neutral tools ffmpeg helper (same
/// probe/poster shape as pager-render, without rebuilding TUI playback).
#[allow(clippy::too_many_arguments)]
pub async fn understand_video(
    video: &VideoContent,
    media: &MediaConfig,
    cache: &ImageDescribeCache,
    store: &MediaDescriptorStore,
    client: Option<xai_grok_inference::InferenceClient>,
    describe_model: Option<&str>,
    provider: Option<&str>,
    frame_route_policy: Result<(), MediaPolicyError>,
    source: ImageDescribeSource,
    runner: &dyn ProcessRunner,
    transcriber: Option<&dyn AsyncAudioTranscriber>,
) -> String {
    let path = video.absolute_path.as_path();
    let policy = auxiliary_media_allowed(media.mode, source);
    let (content_fp, fingerprint_error) = match fingerprint_media_file(path) {
        Ok(fingerprint) => (fingerprint, None),
        Err(error) => (
            fallback_media_fingerprint(path, video.size_bytes, &video.mime_type),
            Some(error),
        ),
    };
    let prompt_fp = stable_describe_prompt_fingerprint();
    let key = MediaDescriptorKey {
        modality: MediaModality::Video,
        content_fingerprint: content_fp,
        source: descriptor_source(source),
        prompt_fingerprint: prompt_fp,
    };
    if let Some(descriptor) = store.get(&key) {
        return render_video_description_block(&descriptor.description);
    }

    let mut sections = Vec::new();
    sections.push(xai_grok_tools::implementations::read_file::video_prompt_summary(video));
    if let Some(error) = fingerprint_error.as_ref() {
        sections.push(format!("Media validation warning: {error}"));
    }

    let probe = ffmpeg::probe_media(path, runner, DEFAULT_PROBE_TIMEOUT).ok();
    let duration = video
        .duration_secs
        .or_else(|| probe.as_ref().and_then(|p| p.duration_secs))
        .unwrap_or(0.0);
    if duration > media.video_max_seconds as f64 {
        sections.push(format!(
            "Note: duration {duration:.1}s exceeds configured video_max_seconds={}; sampling is limited to the configured window.",
            media.video_max_seconds
        ));
    }
    let sample_duration = duration.min(media.video_max_seconds as f64);

    if let Err(error) = policy {
        sections.push(format!("Frame understanding skipped: {error}"));
    } else if let Err(error) = frame_route_policy {
        sections.push(format!("Frame understanding skipped: {error}"));
    } else if let Some(error) = fingerprint_error.as_ref() {
        sections.push(format!("Frame understanding skipped: {error}"));
    } else {
        match ffmpeg::extract_sample_frames_jpeg(
            path,
            sample_duration,
            media.video_max_frames,
            runner,
            DEFAULT_EXTRACT_TIMEOUT,
        ) {
            Ok(frames) => {
                sections.push(format!("Sampled {} frame(s):", frames.len()));
                if let (Some(client), Some(model)) = (client.as_ref(), describe_model) {
                    for (index, frame) in frames.iter().enumerate() {
                        match describe_image(
                            cache,
                            store,
                            client.clone(),
                            model,
                            provider,
                            frame,
                            "image/jpeg",
                            None,
                            "Describe this video frame for the current coding task.",
                            source,
                            Some(path),
                        )
                        .await
                        {
                            Ok(description) => sections.push(format!(
                                "Frame {}:\n{}",
                                index + 1,
                                crate::session::image_describe::scrub_envelope_body(&description)
                            )),
                            Err(error) => sections.push(format!(
                                "Frame {}: [description unavailable: {error}]",
                                index + 1
                            )),
                        }
                    }
                } else {
                    sections.push(
                        "Frame descriptions unavailable: configure an image-capable media route."
                            .to_owned(),
                    );
                }
            }
            Err(error) => sections.push(format!("Frame extraction failed: {error}")),
        }
    }

    if video.has_audio || probe.as_ref().is_some_and(|p| p.has_audio) {
        match (
            stt_allowed_for_source(media.mode, source),
            validate_audio_stt_route(media.audio_model.as_deref()),
        ) {
            (Err(error), _) | (_, Err(error)) => {
                sections.push(format!("Audio track transcript: {error}"));
            }
            (Ok(()), Ok(())) => {
                if let Some(error) = fingerprint_error.as_ref() {
                    sections.push(format!("Audio track transcript skipped: {error}"));
                } else {
                    match ffmpeg::extract_audio_pcm_s16le(
                        path,
                        media.audio_max_seconds.min(media.video_max_seconds),
                        runner,
                        DEFAULT_EXTRACT_TIMEOUT,
                    ) {
                        Ok(pcm) => match transcriber {
                            Some(tx) => {
                                match tx.transcribe_pcm_s16le(&pcm, stt_sample_rate()).await {
                                    Ok(transcript) if !transcript.trim().is_empty() => {
                                        sections.push(format!(
                                        "Audio track transcript (via {XAI_STREAMING_STT_ROUTE}):\n{}",
                                        transcript.trim()
                                    ));
                                    }
                                    Ok(_) => sections.push(format!(
                                        "Audio track transcript: {}",
                                        AudioSttError::EmptyTranscript
                                    )),
                                    Err(error) => {
                                        sections.push(format!("Audio track transcript: {error}"))
                                    }
                                }
                            }
                            None => sections.push(format!(
                                "Audio track transcript: {}",
                                AudioSttError::AuthUnavailable
                            )),
                        },
                        Err(error) => {
                            sections.push(format!("Audio track extraction failed: {error}"));
                        }
                    }
                }
            }
        }
    }

    let mut description = sections.join("\n\n");
    // Frame route may be a vision model id; STT is labeled only in the body.
    match MediaDescriptor::new(
        key,
        description.clone(),
        Some(video.mime_type.clone()),
        describe_model.map(str::to_owned),
        provider.map(str::to_owned),
        path.file_name().and_then(|n| n.to_str()).map(str::to_owned),
    ) {
        Ok(descriptor) => {
            if let Err(error) = store.insert(descriptor) {
                tracing::warn!(%error, "failed to persist video media descriptor");
                description.push_str(&format!(
                    "\n\nDescriptor persistence warning: this video context may be unavailable to later compaction ({error})."
                ));
            }
        }
        Err(error) => {
            tracing::warn!(%error, "failed to validate video media descriptor");
            description.push_str(&format!(
                "\n\nDescriptor persistence warning: this video context could not be recorded ({error})."
            ));
        }
    }
    render_video_description_block(&description)
}

/// Convenience wrapper using the system ffmpeg runner without STT auth.
pub async fn understand_audio_default(
    audio: &AudioContent,
    media: &MediaConfig,
    store: &MediaDescriptorStore,
    source: ImageDescribeSource,
) -> String {
    understand_audio(audio, media, store, source, &SystemProcessRunner, None).await
}

pub fn render_audio_description_block(description: &str) -> String {
    let body = crate::session::image_describe::scrub_envelope_body(description);
    format!(
        "<audio>This is an audio attachment, but instead of playing it, you are given a description/transcript of it.\n\n<audio_description>\n{body}\n</audio_description>\nDon't mention to the user that you only have a description of the audio.</audio>"
    )
}

pub fn render_video_description_block(description: &str) -> String {
    let body = crate::session::image_describe::scrub_envelope_body(description);
    format!(
        "<video>This is a video attachment, but instead of playing it, you are given a description of sampled frames and any available audio transcript.\n\n<video_description>\n{body}\n</video_description>\nDon't mention to the user that you only have a description of the video.</video>"
    )
}

/// Result of persisting one ACP audio attachment under the session assets dir.
#[derive(Debug, Clone)]
pub struct PersistedAudio {
    pub path: std::path::PathBuf,
    pub mime_type: String,
    pub size_bytes: u64,
}

/// Persist ACP audio to `<session_dir>/assets/audio-<uuid>.<ext>` (confined).
fn validate_base64_media_size(encoded_len: usize) -> std::io::Result<()> {
    let estimated_decoded_len = encoded_len.saturating_div(4).saturating_mul(3);
    if estimated_decoded_len as u64 > MAX_MEDIA_INPUT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("audio attachment exceeds the {MAX_MEDIA_INPUT_BYTES}-byte input cap"),
        ));
    }
    Ok(())
}

pub fn persist_user_audio(
    session_dir: &Path,
    data_b64: &str,
    mime_type: &str,
) -> std::io::Result<PersistedAudio> {
    use base64::Engine as _;
    validate_base64_media_size(data_b64.len())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_b64)
        .map_err(|e| std::io::Error::other(format!("base64 decode: {e}")))?;
    if bytes.len() as u64 > MAX_MEDIA_INPUT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("audio attachment exceeds the {MAX_MEDIA_INPUT_BYTES}-byte input cap"),
        ));
    }
    let assets_dir = session_dir.join("assets");
    std::fs::create_dir_all(&assets_dir)?;
    let ext = audio_mime_to_extension(mime_type);
    let filename = format!("audio-{}.{ext}", uuid::Uuid::new_v4());
    let path = assets_dir.join(&filename);
    // Reject escape attempts: path must stay under assets/.
    if path.strip_prefix(&assets_dir).ok().is_none_or(|rel| {
        rel.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    }) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "audio asset path escaped session assets directory",
        ));
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    std::io::Write::write_all(&mut file, &bytes)?;
    Ok(PersistedAudio {
        path,
        mime_type: mime_type.to_owned(),
        size_bytes: bytes.len() as u64,
    })
}

fn audio_mime_to_extension(mime: &str) -> &'static str {
    match mime {
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => "m4a",
        "audio/ogg" | "audio/opus" => "ogg",
        "audio/flac" => "flac",
        "audio/aac" => "aac",
        _ => "bin",
    }
}

/// Persist ACP audio to the session assets dir and convert to a text envelope.
///
/// Never returns or persists a conversation Audio content variant.
pub async fn normalize_acp_audio_to_envelope(
    session_dir: &Path,
    data_b64: &str,
    mime_type: &str,
    media: &MediaConfig,
    store: &MediaDescriptorStore,
    source: ImageDescribeSource,
    runner: &dyn ProcessRunner,
    transcriber: Option<&dyn AsyncAudioTranscriber>,
) -> String {
    let persisted = match persist_user_audio(session_dir, data_b64, mime_type) {
        Ok(p) => p,
        Err(error) => {
            return render_audio_description_block(&format!(
                "Audio attachment could not be saved to the session assets directory: {error}"
            ));
        }
    };
    let audio = AudioContent {
        absolute_path: persisted.path.clone(),
        mime_type: persisted.mime_type,
        size_bytes: persisted.size_bytes,
        duration_secs: None,
        sample_rate: None,
        channels: None,
    };
    let mut envelope = understand_audio(&audio, media, store, source, runner, transcriber).await;
    // Surface the confined session-relative asset path for coding tasks.
    if let Some(rel) = persisted
        .path
        .strip_prefix(session_dir)
        .ok()
        .and_then(|p| p.to_str())
    {
        envelope = format!("{envelope}\n<audio_asset>{rel}</audio_asset>");
    }
    envelope
}

/// Freeze one immutable descriptor map for a compaction / recap / flush attempt.
///
/// Existing store entries for conversation image fingerprints are always folded
/// in. When `allow_lazy_backfill` is true and a client/model is supplied, up to
/// `image_limit` missing images are described once and persisted to the store
/// before the map is sealed. Failures are fail-soft: that fingerprint simply
/// remains absent and the summarizer path uses the placeholder.
pub async fn freeze_compaction_media_descriptors(
    cache: &ImageDescribeCache,
    store: &MediaDescriptorStore,
    conversation: &[xai_grok_inference_types::ConversationItem],
    allow_lazy_backfill: bool,
    client: Option<xai_grok_inference::InferenceClient>,
    model: Option<&str>,
    provider: Option<&str>,
    image_limit: usize,
) -> std::sync::Arc<xai_chat_state::compaction_utils::CompactionMediaDescriptors> {
    use xai_chat_state::compaction_utils::{
        CompactionMediaDescriptors, decode_image_data_url, image_url_content_fingerprint,
    };
    use xai_grok_inference_types::{ContentPart, ConversationItem};

    let mut map = CompactionMediaDescriptors::empty();
    let store_snapshot = store.snapshot();

    let collect_image_urls = |item: &ConversationItem| -> Vec<String> {
        match item {
            ConversationItem::User(user) => user
                .content
                .iter()
                .filter_map(|part| match part {
                    ContentPart::Image { url } => Some(url.to_string()),
                    _ => None,
                })
                .collect(),
            ConversationItem::ToolResult(result) => result
                .images
                .iter()
                .filter_map(|part| match part {
                    ContentPart::Image { url } => Some(url.to_string()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    };

    // Seed from durable store entries that match conversation fingerprints.
    for item in conversation {
        for url in collect_image_urls(item) {
            let Some(fp) = image_url_content_fingerprint(&url) else {
                continue;
            };
            if map.get(&fp).is_some() {
                continue;
            }
            if let Some(descriptor) = store_snapshot.values().find(|entry| {
                entry.key.modality == MediaModality::Image && entry.key.content_fingerprint == fp
            }) {
                map.insert(fp, descriptor.description.clone());
            }
        }
    }

    if !allow_lazy_backfill {
        return std::sync::Arc::new(map);
    }
    let (Some(client), Some(model)) = (client, model) else {
        return std::sync::Arc::new(map);
    };

    let mut backfilled = 0usize;
    let limit = image_limit.max(1);
    for item in conversation {
        if backfilled >= limit {
            break;
        }
        for url in collect_image_urls(item) {
            if backfilled >= limit {
                break;
            }
            let Some(fp) = image_url_content_fingerprint(&url) else {
                continue;
            };
            if map.get(&fp).is_some() {
                continue;
            }
            let Some(bytes) = decode_image_data_url(&url) else {
                continue;
            };
            let mime = url
                .strip_prefix("data:")
                .and_then(|rest| rest.split_once(';').map(|(mime, _)| mime))
                .filter(|mime| !mime.is_empty())
                .unwrap_or("image/png");
            match describe_image(
                cache,
                store,
                client.clone(),
                model,
                provider,
                &bytes,
                mime,
                None,
                "",
                ImageDescribeSource::CompactionBackfill,
                None,
            )
            .await
            {
                Ok(description) => {
                    map.insert(fp, description);
                    backfilled += 1;
                }
                Err(error) => {
                    tracing::debug!(
                        target: "media_pipeline",
                        error = %error,
                        fingerprint = %fp,
                        "lazy compaction image describe failed; placeholder will be used"
                    );
                }
            }
        }
    }

    std::sync::Arc::new(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use xai_grok_tools::util::ffmpeg::{FfmpegError, ProcessOutput};

    struct MockRunner {
        responses: Mutex<VecDeque<Result<ProcessOutput, FfmpegError>>>,
    }

    impl MockRunner {
        fn new(responses: Vec<Result<ProcessOutput, FfmpegError>>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
            }
        }
    }

    impl ProcessRunner for MockRunner {
        fn run(
            &self,
            _program: &str,
            _args: &[&str],
            _timeout: std::time::Duration,
        ) -> Result<ProcessOutput, FfmpegError> {
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Err(FfmpegError::EmptyOutput))
        }
    }

    #[test]
    fn auxiliary_policy_gates_user_media_but_allows_tool_reads() {
        use crate::config::MediaMode;

        assert!(
            auxiliary_media_allowed(MediaMode::Auto, ImageDescribeSource::UserAttachment).is_ok()
        );
        assert!(
            auxiliary_media_allowed(MediaMode::ToolsOnly, ImageDescribeSource::ToolRead).is_ok()
        );
        assert!(matches!(
            auxiliary_media_allowed(MediaMode::ToolsOnly, ImageDescribeSource::UserAttachment),
            Err(MediaPolicyError::UserMediaSkippedByMode)
        ));
        assert!(matches!(
            auxiliary_media_allowed(MediaMode::Off, ImageDescribeSource::ToolRead),
            Err(MediaPolicyError::Disabled)
        ));
        assert!(matches!(
            auxiliary_media_allowed(
                MediaMode::ToolsOnly,
                ImageDescribeSource::CompactionBackfill
            ),
            Err(MediaPolicyError::UserMediaSkippedByMode)
        ));
    }

    #[test]
    fn auxiliary_provider_policy_blocks_external_routes_for_zdr_teams() {
        use xai_grok_inference::config::ProviderIdentity;

        let zdr_auth = crate::auth::GrokAuth {
            team_blocked_reasons: vec!["BLOCKED_REASON_NO_LOGS".to_owned()],
            ..crate::auth::GrokAuth::test_default()
        };
        assert!(auxiliary_media_provider_allowed(ProviderIdentity::Xai, Some(&zdr_auth)).is_ok());
        assert!(matches!(
            auxiliary_media_provider_allowed(ProviderIdentity::OpenRouter, Some(&zdr_auth)),
            Err(MediaPolicyError::ExternalProviderBlockedByZdr { .. })
        ));
        assert!(
            auxiliary_media_provider_allowed(ProviderIdentity::Anthropic, None).is_ok(),
            "without trusted ZDR metadata, normal route/privacy controls still apply"
        );
    }

    #[tokio::test]
    async fn understand_video_surfaces_denied_frame_route_without_false_provenance() {
        use xai_grok_inference::config::ProviderIdentity;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.mp4");
        std::fs::write(&path, b"video").unwrap();
        let store = MediaDescriptorStore::empty(dir.path());
        let video = VideoContent {
            absolute_path: path,
            mime_type: "video/mp4".to_owned(),
            size_bytes: 5,
            duration_secs: Some(1.0),
            width: Some(640),
            height: Some(360),
            has_audio: false,
        };
        let runner = MockRunner::new(vec![
            Err(FfmpegError::ToolMissing { tool: "ffprobe" }),
            Ok(ProcessOutput {
                stdout: Vec::new(),
                stderr: Vec::new(),
                status_code: 0,
            }),
        ]);
        let denied = Err(MediaPolicyError::ExternalProviderBlockedByZdr {
            provider: ProviderIdentity::OpenRouter.label(),
        });
        let text = understand_video(
            &video,
            &MediaConfig::default(),
            &ImageDescribeCache::new(),
            &store,
            None,
            None,
            None,
            denied,
            ImageDescribeSource::ToolRead,
            &runner,
            None,
        )
        .await;

        assert!(text.contains("ZDR policy blocks auxiliary media disclosure to OpenRouter"));
        let descriptor = store
            .snapshot()
            .values()
            .next()
            .cloned()
            .expect("video descriptor should be persisted");
        assert!(descriptor.model_id.is_none());
        assert!(descriptor.provider.is_none());
    }

    #[tokio::test]
    async fn understand_audio_uses_transcript_when_stt_available() {
        use crate::session::media_stt::MockAudioTranscriber;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.wav");
        std::fs::write(&path, b"RIFF....WAVE").unwrap();
        let store = MediaDescriptorStore::empty(dir.path());
        let audio = AudioContent {
            absolute_path: path,
            mime_type: "audio/wav".to_owned(),
            size_bytes: 12,
            duration_secs: Some(1.0),
            sample_rate: Some(16_000),
            channels: Some(1),
        };
        let runner = MockRunner::new(vec![
            Ok(ProcessOutput {
                stdout: br#"{"format":{"duration":"1.0"},"streams":[{"codec_type":"audio"}]}"#
                    .to_vec(),
                stderr: Vec::new(),
                status_code: 0,
            }),
            Ok(ProcessOutput {
                stdout: b"\0\0\0\0pcm".to_vec(),
                stderr: Vec::new(),
                status_code: 0,
            }),
        ]);
        let stt = MockAudioTranscriber {
            result: Ok("hello world".into()),
        };
        let text = understand_audio(
            &audio,
            &MediaConfig::default(),
            &store,
            ImageDescribeSource::ToolRead,
            &runner,
            Some(&stt),
        )
        .await;
        assert!(text.contains("<audio_description>"));
        assert!(text.contains("hello world"));
        assert!(!text.contains("base64,"));
    }

    #[tokio::test]
    async fn understand_audio_failure_is_explicit_not_silent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.wav");
        std::fs::write(&path, b"RIFF....WAVE").unwrap();
        let store = MediaDescriptorStore::empty(dir.path());
        let audio = AudioContent {
            absolute_path: path,
            mime_type: "audio/wav".to_owned(),
            size_bytes: 12,
            duration_secs: None,
            sample_rate: None,
            channels: None,
        };
        let runner = MockRunner::new(vec![
            Err(FfmpegError::ToolMissing { tool: "ffprobe" }),
            Err(FfmpegError::ToolMissing { tool: "ffmpeg" }),
        ]);
        let text = understand_audio(
            &audio,
            &MediaConfig::default(),
            &store,
            ImageDescribeSource::ToolRead,
            &runner,
            None,
        )
        .await;
        assert!(text.contains("Audio extraction failed") || text.contains("Transcript:"));
        assert!(text.contains("<audio>"));
    }

    #[tokio::test]
    async fn acp_audio_normalizes_to_asset_and_transcript_envelope() {
        use crate::session::media_stt::MockAudioTranscriber;
        use base64::Engine as _;
        let dir = tempfile::tempdir().unwrap();
        let store = MediaDescriptorStore::empty(dir.path());
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"RIFFWAVE");
        let runner = MockRunner::new(vec![
            Ok(ProcessOutput {
                stdout: br#"{"format":{"duration":"0.5"},"streams":[{"codec_type":"audio"}]}"#
                    .to_vec(),
                stderr: Vec::new(),
                status_code: 0,
            }),
            Ok(ProcessOutput {
                stdout: b"\0\0pcm".to_vec(),
                stderr: Vec::new(),
                status_code: 0,
            }),
        ]);
        let stt = MockAudioTranscriber {
            result: Ok("from acp".into()),
        };
        let text = normalize_acp_audio_to_envelope(
            dir.path(),
            &b64,
            "audio/wav",
            &MediaConfig::default(),
            &store,
            ImageDescribeSource::UserAttachment,
            &runner,
            Some(&stt),
        )
        .await;
        assert!(text.contains("from acp"));
        assert!(text.contains("<audio_asset>assets/audio-"));
        assert!(!text.contains("bytes_b64="));
        let mut assets = std::fs::read_dir(dir.path().join("assets")).unwrap();
        let asset = assets.next().unwrap().unwrap().path();
        assert!(assets.next().is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(asset).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn media_fingerprint_reader_stops_at_running_byte_cap() {
        let reader = std::io::repeat(0).take(MAX_MEDIA_INPUT_BYTES + 1);
        let error = fingerprint_media_reader(reader).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[cfg(unix)]
    #[test]
    fn media_fingerprint_rejects_oversized_sparse_file_without_reading_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oversized.wav");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_MEDIA_INPUT_BYTES + 1).unwrap();
        let error = fingerprint_media_file(&path).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn acp_audio_rejects_oversized_base64_before_decode_or_write() {
        let encoded_len = ((MAX_MEDIA_INPUT_BYTES as usize + 1) / 3)
            .saturating_mul(4)
            .saturating_add(4);
        let error = validate_base64_media_size(encoded_len).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn tools_only_mode_skips_user_attachment_stt() {
        use crate::session::media_stt::MockAudioTranscriber;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.wav");
        std::fs::write(&path, b"RIFF....WAVE").unwrap();
        let store = MediaDescriptorStore::empty(dir.path());
        let audio = AudioContent {
            absolute_path: path,
            mime_type: "audio/wav".to_owned(),
            size_bytes: 12,
            duration_secs: None,
            sample_rate: None,
            channels: None,
        };
        let runner = MockRunner::new(vec![Ok(ProcessOutput {
            stdout: br#"{"format":{"duration":"1.0"},"streams":[{"codec_type":"audio"}]}"#.to_vec(),
            stderr: Vec::new(),
            status_code: 0,
        })]);
        let stt = MockAudioTranscriber {
            result: Ok("should not run".into()),
        };
        let mut media = MediaConfig::default();
        media.mode = crate::config::MediaMode::ToolsOnly;
        let text = understand_audio(
            &audio,
            &media,
            &store,
            ImageDescribeSource::UserAttachment,
            &runner,
            Some(&stt),
        )
        .await;
        assert!(text.contains("tools_only") || text.contains("not transcribed"));
        assert!(!text.contains("should not run"));
    }

    #[tokio::test]
    async fn freeze_compaction_descriptors_seeds_from_store_without_media_calls() {
        use xai_chat_state::compaction_utils::{
            compaction_image_content_fingerprint,
            prepare_conversation_for_summarization_with_descriptors,
        };
        use xai_grok_inference_types::ConversationItem;

        let dir = tempfile::tempdir().unwrap();
        let store = MediaDescriptorStore::empty(dir.path());
        let raw = b"phase3-compact-image";
        let fp = content_fingerprint(raw);
        store
            .insert(
                MediaDescriptor::new(
                    MediaDescriptorKey {
                        modality: MediaModality::Image,
                        content_fingerprint: fp.clone(),
                        source: MediaDescriptorSource::UserAttachment,
                        prompt_fingerprint: "d".repeat(64),
                    },
                    "stacked red error toast".to_owned(),
                    Some("image/png".to_owned()),
                    Some("m".to_owned()),
                    Some("xai".to_owned()),
                    None,
                )
                .unwrap(),
            )
            .unwrap();

        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, raw);
        let mut user = ConversationItem::user("what failed?");
        user.add_image(format!("data:image/png;base64,{b64}"));
        let conversation = vec![user];

        // allow_lazy_backfill=true but no client/model ⇒ store seed only, no media call.
        let map = freeze_compaction_media_descriptors(
            &ImageDescribeCache::new(),
            &store,
            &conversation,
            true,
            None,
            None,
            None,
            8,
        )
        .await;
        assert_eq!(
            map.get(&fp),
            Some("stacked red error toast"),
            "store description must fold by content fingerprint"
        );
        assert_eq!(
            map.get(&compaction_image_content_fingerprint(raw)),
            Some("stacked red error toast")
        );

        let prepared = prepare_conversation_for_summarization_with_descriptors(conversation, &map);
        assert!(
            prepared[0]
                .text_content()
                .contains("stacked red error toast")
        );
        assert!(!prepared[0].text_content().contains("base64,"));
    }
}
