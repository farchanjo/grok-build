//! Shell-owned file-audio transcription over the xAI streaming STT transport.
//!
//! Uses [`xai_grok_voice::stt::StreamingSttSession`] with live auth refresh.
//! There is no adapter-native / catalog "audio model" path: when STT runs, the
//! route label is always [`XAI_STREAMING_STT_ROUTE`]. Microphone capture is
//! never used here (`xai-grok-voice` is linked with `default-features = false`).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use xai_grok_voice::auth::{SharedVoiceAuth, VoiceAuthProvider};
use xai_grok_voice::config::{DEFAULT_SAMPLE_RATE, VoiceConfig};
use xai_grok_voice::stt::{StreamingSttEvent, StreamingSttSession};

use crate::config::MediaMode;
use crate::session::image_describe::ImageDescribeSource;

/// Truthful route label written into media descriptors when xAI streaming STT
/// produced a transcript. Never a generic catalog model id.
pub const XAI_STREAMING_STT_ROUTE: &str = "xai-streaming-stt";

/// Wall-clock budget for one full connect + stream + drain cycle.
pub const DEFAULT_STT_TIMEOUT: Duration = Duration::from_secs(90);
/// Per-chunk send size (~100 ms of 16 kHz mono s16le).
const PCM_CHUNK_BYTES: usize = 3_200;

/// Errors from the shell-owned file STT path.
#[derive(Debug, thiserror::Error)]
pub enum AudioSttError {
    #[error("media understanding is disabled ([media].mode = off)")]
    Disabled,
    #[error(
        "user audio is not transcribed when [media].mode = tools_only (use read_file on the asset)"
    )]
    UserAudioSkippedByMode,
    #[error("no usable auth for xAI streaming STT (sign in or set an xAI API key)")]
    AuthUnavailable,
    #[error(
        "audio route {0:?} is not supported; only xAI streaming STT ({XAI_STREAMING_STT_ROUTE}) is implemented"
    )]
    UnsupportedRoute(String),
    #[error("xAI streaming STT timed out")]
    Timeout,
    #[error("xAI streaming STT returned an empty transcript")]
    EmptyTranscript,
    #[error("xAI streaming STT transport error: {0}")]
    Transport(String),
}

/// Async file-audio transcriber. Production uses xAI streaming STT; tests inject
/// a mock. Never logs bearer material.
pub trait AsyncAudioTranscriber: Send + Sync {
    fn transcribe_pcm_s16le<'a>(
        &'a self,
        pcm: &'a [u8],
        sample_rate: u32,
    ) -> Pin<Box<dyn Future<Output = Result<String, AudioSttError>> + Send + 'a>>;
}

/// Whether STT may run for this media-understanding source under `[media].mode`.
pub fn stt_allowed_for_source(
    mode: MediaMode,
    source: ImageDescribeSource,
) -> Result<(), AudioSttError> {
    match mode {
        MediaMode::Off => Err(AudioSttError::Disabled),
        MediaMode::ToolsOnly => match source {
            ImageDescribeSource::ToolRead | ImageDescribeSource::CompactionBackfill => Ok(()),
            ImageDescribeSource::UserAttachment => Err(AudioSttError::UserAudioSkippedByMode),
        },
        MediaMode::Auto => Ok(()),
    }
}

/// Validate `[media].audio_model` against the only implemented STT route.
///
/// `None` / empty / explicit xAI STT aliases are accepted. Any other catalog
/// model id fails closed with a clear message (no silent fake inference).
///
/// Production also requires [`crate::session::auxiliary_route::resolve_media_stt_route`]
/// so the frozen session route is exact xAI before any AuthManager bearer is used.
pub fn validate_audio_stt_route(audio_model: Option<&str>) -> Result<(), AudioSttError> {
    match audio_model.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(()),
        Some(p)
            if crate::session::auxiliary_route::MEDIA_STT_ROUTE_ALIASES
                .iter()
                .any(|a| *a == p)
                || p == XAI_STREAMING_STT_ROUTE =>
        {
            Ok(())
        }
        Some(other) => Err(AudioSttError::UnsupportedRoute(other.to_owned())),
    }
}

/// Map a MediaStt aux-route failure to an [`AudioSttError`] (fail closed).
pub fn audio_stt_error_from_aux(
    err: crate::session::auxiliary_route::AuxiliaryRouteError,
) -> AudioSttError {
    use crate::session::auxiliary_route::AuxiliaryRouteError;
    match err {
        AuxiliaryRouteError::ExplicitPinFailed { selection }
        | AuxiliaryRouteError::Missing { input: selection }
        | AuxiliaryRouteError::NamespacedHijackRejected { input: selection }
        | AuxiliaryRouteError::Ambiguous { input: selection } => {
            AudioSttError::UnsupportedRoute(selection)
        }
        AuxiliaryRouteError::CredentialUnavailable { .. }
        | AuxiliaryRouteError::SessionRouteRequired => AudioSttError::AuthUnavailable,
        AuxiliaryRouteError::ConstructionFailed { detail, .. } => {
            // Non-xAI session pin: refuse sibling/current bearer use.
            AudioSttError::Transport(detail)
        }
    }
}

/// Adapts the shell [`ApiKeyProvider`] onto voice's [`VoiceAuthProvider`].
struct ApiKeyVoiceAuth(xai_grok_tools::types::SharedApiKeyProvider);

impl std::fmt::Debug for ApiKeyVoiceAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKeyVoiceAuth")
    }
}

impl VoiceAuthProvider for ApiKeyVoiceAuth {
    fn bearer(&self) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>> {
        let provider = self.0.clone();
        Box::pin(async move { provider.current_api_key_async().await })
    }
}

/// Build a refreshing voice auth handle from the session `AuthManager`.
pub fn voice_auth_from_manager(auth_manager: Arc<crate::auth::AuthManager>) -> SharedVoiceAuth {
    Arc::new(ApiKeyVoiceAuth(crate::auth::shared_api_key_provider(
        auth_manager,
    )))
}

/// Build voice auth from an existing shared API key provider (tests / spawn).
pub fn voice_auth_from_api_key_provider(
    provider: xai_grok_tools::types::SharedApiKeyProvider,
) -> SharedVoiceAuth {
    Arc::new(ApiKeyVoiceAuth(provider))
}

/// Production transcriber: xAI `wss://…/v1/stt` with live bearer resolution.
pub struct XaiStreamingAudioTranscriber {
    auth: SharedVoiceAuth,
    config: VoiceConfig,
    timeout: Duration,
}

impl XaiStreamingAudioTranscriber {
    pub fn new(auth: SharedVoiceAuth, config: VoiceConfig) -> Self {
        Self {
            auth,
            config,
            timeout: DEFAULT_STT_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Prefer the session xAI API base when present so enterprise proxies work.
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        let base = api_base.into().trim().trim_end_matches('/').to_owned();
        if !base.is_empty() {
            self.config.api_base = base;
        }
        self
    }
}

impl AsyncAudioTranscriber for XaiStreamingAudioTranscriber {
    fn transcribe_pcm_s16le<'a>(
        &'a self,
        pcm: &'a [u8],
        sample_rate: u32,
    ) -> Pin<Box<dyn Future<Output = Result<String, AudioSttError>> + Send + 'a>> {
        Box::pin(async move {
            if pcm.is_empty() {
                return Err(AudioSttError::EmptyTranscript);
            }
            let mut config = self.config.clone();
            if sample_rate > 0 {
                config.sample_rate = sample_rate;
            }
            // Resolve bearer at connect time so long sessions follow refresh.
            let bearer = self
                .auth
                .bearer()
                .await
                .filter(|b| !b.trim().is_empty())
                .ok_or(AudioSttError::AuthUnavailable)?;

            let work = async {
                let mut session = StreamingSttSession::connect(&config, &bearer)
                    .await
                    .map_err(|e| AudioSttError::Transport(e.to_string()))?;
                // Never log bearer; only pcm length for diagnostics.
                tracing::debug!(
                    pcm_bytes = pcm.len(),
                    sample_rate = config.sample_rate,
                    "streaming file audio to xAI STT"
                );
                for chunk in pcm.chunks(PCM_CHUNK_BYTES) {
                    session
                        .send_pcm(chunk.to_vec())
                        .await
                        .map_err(|e| AudioSttError::Transport(e.to_string()))?;
                }
                session.finish_audio();

                let mut last_final = String::new();
                while let Some(event) = session.recv().await {
                    match event {
                        StreamingSttEvent::Done { text } => {
                            let text = text.trim().to_owned();
                            if text.is_empty() {
                                return Err(AudioSttError::EmptyTranscript);
                            }
                            return Ok(text);
                        }
                        StreamingSttEvent::Partial(partial) if partial.is_final => {
                            last_final = partial.text;
                        }
                        StreamingSttEvent::Error { message } => {
                            // Never include auth material from upstream in logs
                            // at info; surface a sanitized transport error.
                            return Err(AudioSttError::Transport(message));
                        }
                        StreamingSttEvent::Ready | StreamingSttEvent::Partial(_) => {}
                    }
                }
                let text = last_final.trim().to_owned();
                if text.is_empty() {
                    Err(AudioSttError::EmptyTranscript)
                } else {
                    Ok(text)
                }
            };

            match tokio::time::timeout(self.timeout, work).await {
                Ok(result) => result,
                Err(_) => Err(AudioSttError::Timeout),
            }
        })
    }
}

/// Test/mock transcriber with a fixed response or error.
#[derive(Debug, Clone)]
pub struct MockAudioTranscriber {
    pub result: Result<String, String>,
}

impl AsyncAudioTranscriber for MockAudioTranscriber {
    fn transcribe_pcm_s16le<'a>(
        &'a self,
        _pcm: &'a [u8],
        _sample_rate: u32,
    ) -> Pin<Box<dyn Future<Output = Result<String, AudioSttError>> + Send + 'a>> {
        let result = self.result.clone();
        Box::pin(async move {
            match result {
                Ok(text) if text.trim().is_empty() => Err(AudioSttError::EmptyTranscript),
                Ok(text) => Ok(text),
                Err(message) if message == "auth" => Err(AudioSttError::AuthUnavailable),
                Err(message) if message == "timeout" => Err(AudioSttError::Timeout),
                Err(message) => Err(AudioSttError::Transport(message)),
            }
        })
    }
}

/// Build a production STT handle when auth is available; `None` otherwise.
///
/// File STT follows the same endpoint precedence as live voice: an explicit
/// `[voice].api_base`, then `[endpoints].xai_api_base_url`, then the caller's
/// resolved endpoint (including managed/env overrides), then the xAI default.
pub fn maybe_xai_stt_transcriber(
    auth_manager: Option<&Arc<crate::auth::AuthManager>>,
    api_key_provider: Option<&xai_grok_tools::types::SharedApiKeyProvider>,
    config: VoiceConfig,
) -> Option<Arc<dyn AsyncAudioTranscriber>> {
    let auth = if let Some(am) = auth_manager {
        voice_auth_from_manager(am.clone())
    } else if let Some(provider) = api_key_provider {
        voice_auth_from_api_key_provider(provider.clone())
    } else {
        return None;
    };
    Some(Arc::new(XaiStreamingAudioTranscriber::new(auth, config)))
}

/// Default sample rate for extracted file audio (matches voice crate).
pub fn stt_sample_rate() -> u32 {
    DEFAULT_SAMPLE_RATE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_validation_accepts_xai_aliases_only() {
        assert!(validate_audio_stt_route(None).is_ok());
        assert!(validate_audio_stt_route(Some("xai-streaming-stt")).is_ok());
        assert!(validate_audio_stt_route(Some("@session")).is_ok());
        let err = validate_audio_stt_route(Some("some-catalog-audio-model")).unwrap_err();
        assert!(matches!(err, AudioSttError::UnsupportedRoute(_)));
        assert!(!err.to_string().contains("OpenRouter"));
    }

    #[test]
    fn mode_gates_user_vs_tool() {
        assert!(
            stt_allowed_for_source(MediaMode::Auto, ImageDescribeSource::UserAttachment).is_ok()
        );
        assert!(stt_allowed_for_source(MediaMode::Auto, ImageDescribeSource::ToolRead).is_ok());
        assert!(
            stt_allowed_for_source(MediaMode::ToolsOnly, ImageDescribeSource::ToolRead).is_ok()
        );
        assert!(matches!(
            stt_allowed_for_source(MediaMode::ToolsOnly, ImageDescribeSource::UserAttachment),
            Err(AudioSttError::UserAudioSkippedByMode)
        ));
        assert!(matches!(
            stt_allowed_for_source(MediaMode::Off, ImageDescribeSource::ToolRead),
            Err(AudioSttError::Disabled)
        ));
    }

    #[test]
    fn transcriber_preserves_resolved_voice_endpoint() {
        #[derive(Debug)]
        struct NoAuth;
        impl VoiceAuthProvider for NoAuth {
            fn bearer(&self) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>> {
                Box::pin(async { None })
            }
        }
        let mut config = VoiceConfig::default();
        config.api_base = "https://enterprise.example/xai/v1".to_owned();
        let transcriber = XaiStreamingAudioTranscriber::new(Arc::new(NoAuth), config);
        assert_eq!(
            transcriber.config.api_base,
            "https://enterprise.example/xai/v1"
        );
    }

    #[tokio::test]
    async fn mock_transcriber_returns_text() {
        let mock = MockAudioTranscriber {
            result: Ok("hello from mock".into()),
        };
        let text = mock.transcribe_pcm_s16le(&[0u8; 64], 16_000).await.unwrap();
        assert_eq!(text, "hello from mock");
    }

    #[tokio::test]
    async fn mock_transcriber_auth_failure_is_explicit() {
        let mock = MockAudioTranscriber {
            result: Err("auth".into()),
        };
        let err = mock
            .transcribe_pcm_s16le(&[0u8; 8], 16_000)
            .await
            .unwrap_err();
        assert!(matches!(err, AudioSttError::AuthUnavailable));
    }
}
