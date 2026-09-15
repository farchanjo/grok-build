//! `image_gen` tool — generates images via the xAI Imagine API and saves
//! them to the local filesystem so the model can reference them in code
//! (e.g. `<img src="images/hero.jpg">`).
//!
//! Architecture follows the same pattern as `web_search`:
//!
//! - [`ImageGenConfig`] is built from session credentials by the host and
//!   injected into the tool registry.
//! - When `Enabled`, an [`ImageGenClient`] is constructed once and injected
//!   into `Resources`. The tool reads it at runtime via `resources.require()`.
//! - When `Disabled`, the tool is not registered so the model never sees it.
//!
//! The generated image is written to `<session_folder>/images/<n>.jpg`
//! where `<n>` is a session-scoped counter (1, 2, 3, ... — 1 token each).
//! The tool returns the absolute path so the model can copy or move the
//! image into the project working directory when it needs a persistent asset.
//!
//! The endpoint is resolved per surface (`GROK_IMAGE_BASE_URL` /
//! `[tools.image_gen] base_url` for generation, `GROK_IMAGE_EDIT_BASE_URL` /
//! `[tools.image_edit] base_url` for edits, both defaulting to
//! `[endpoints].xai_api_base_url`), so imagine can point at a gateway while
//! chat goes elsewhere. The response decoder accepts the xAI and the OpenAI
//! envelope; `provider = "openai"` additionally sends the OpenAI request
//! fields instead of the xAI `aspect_ratio` / `resolution` pair. See
//! [`super::media_endpoint`].

use base64::Engine as _;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};

use super::media_endpoint::{
    MediaProvider, MediaSurface, missing_route_message, openai_size_for_aspect_ratio,
    status_is_missing_route, unsupported_surface_message,
};
use crate::attribution::{SharedAttributionCallback, ToolConsumer};
use crate::types::SharedApiKeyProvider;

use crate::types::output::{MediaGenOutput, ToolOutput};
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::resources::SessionFolder;
use crate::types::tool::{ToolKind, ToolNamespace};

/// Default Imagine model for `image_gen`. Used unless an explicit
/// `model_override` is supplied via `ImageGenConfig::Enabled`.
const XAI_IMAGINE_MODEL: &str = "grok-imagine-image-quality";
// Some Imagine models (e.g. `grok-imagine-image`, selectable via `model_override`)
// expand the prompt then generate, and the proxy buffers
// the whole image before sending any bytes — so the client may receive nothing
// for well over a minute. Keep these generous so a slow-but-progressing
// generation isn't cut off.
const IMAGE_GEN_TIMEOUT_SECS: u64 = 300;
const IMAGE_GEN_READ_TIMEOUT_SECS: u64 = 240;
const DEFAULT_IMAGE_DIR: &str = "images";

pub use xai_grok_tools_api::slash_commands::{
    IMAGE_GEN_TOOL_NAME, IMAGINE_COMMAND_NAME, imagine_instruction, imagine_usage_message,
};

/// Prose returned to the model (as a normal, successful tool result) when a
/// free / X Basic user calls `image_gen` or `image_edit`. The model relays it
/// to the user. The deliberate `/imagine` slash command shows the richer
/// SuperGrok upsell modal instead; this covers the natural-language path.
pub(crate) const TIER_RESTRICTED_UPSELL: &str = "Image generation is a SuperGrok feature and isn't available on the free or X Basic tier. Let the user know they can unlock image and video generation by upgrading to SuperGrok: https://grok.com/supergrok?referrer=grok-build. Do not retry this tool.";

/// HTTP client for xAI Imagine API. Cloned per-request; shares `Arc` state.
#[derive(Clone)]
pub struct ImageGenClient {
    http: reqwest::Client,
    base_url: String,
    /// Base URL used by `image_edit` (`/images/edits`). Same as `base_url`
    /// unless `[tools.image_edit] base_url` / `GROK_IMAGE_EDIT_BASE_URL`
    /// points that surface somewhere else.
    edit_base_url: String,
    /// Imagine model slug used by `generate()`. Selected at construction
    /// from `ImageGenConfig::model_override` (falling back to
    /// [`XAI_IMAGINE_MODEL`]). `image_edit` uses its own model and is
    /// unaffected.
    model: String,
    /// Model slug used by `image_edit`; its own default unless
    /// `[tools.image_edit] model` / `GROK_IMAGE_EDIT_MODEL` overrides it.
    edit_model: String,
    /// Wire shape for `/images/*` requests. `Auto`/`Xai` keep the historical
    /// payload byte-for-byte; `OpenAi` drops the xAI-only fields.
    provider: MediaProvider,
    /// Wire shape for `/images/edits` requests. Kept separate from `provider`
    /// so `[tools.image_edit] provider` can silence (or reshape) the edit
    /// surface without touching generation; it is seeded from `provider` when
    /// the edit surface has no key of its own.
    edit_provider: MediaProvider,
    writer: super::storage::SessionFileWriter,
    api_key_provider: Option<SharedApiKeyProvider>,
    /// Optional 401-attribution hook. Hosts wire this so a 401 from the
    /// Imagine API emits an `auth_401_attribution` event with
    /// `consumer == "ImageGen"` for unified auth-failure telemetry.
    attribution_callback: Option<SharedAttributionCallback>,
    /// When `true`, the user is on a tier the Imagine server zero-limits
    /// (free / X Basic). `image_gen` / `image_edit` short-circuit before any
    /// HTTP call and return the SuperGrok upsell prose instead. See
    /// [`ImageGenClient::is_tier_restricted`].
    tier_restricted: bool,
}

impl ImageGenClient {
    pub fn new(
        config: &ImageGenConfig,
        api_key_provider: Option<SharedApiKeyProvider>,
    ) -> Result<Self, xai_tool_runtime::ToolError> {
        let ImageGenConfig::Enabled {
            api_key,
            base_url,
            edit_base_url,
            extra_headers,
            model_override,
            edit_model_override,
            provider,
            edit_provider,
            tier_restricted,
            ..
        } = config
        else {
            return Err(xai_tool_runtime::ToolError::invalid_arguments(
                "Cannot create ImageGenClient from disabled config",
            ));
        };
        let model = model_override
            .clone()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| XAI_IMAGINE_MODEL.to_owned());
        let edit_model = edit_model_override
            .clone()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| super::image_edit::XAI_IMAGINE_EDIT_MODEL.to_owned());
        let edit_base_url = edit_base_url
            .clone()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| base_url.clone());

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        // Always bake the static api_key as the default Authorization header.
        // The dynamic provider overrides per-request; this is the fallback.
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|e| {
                xai_tool_runtime::ToolError::invalid_arguments(format!(
                    "Invalid API key for header: {e}"
                ))
            })?,
        );

        extra_headers.into_iter().try_for_each(|(key, value)| {
            let header_name =
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    xai_tool_runtime::ToolError::invalid_arguments(format!(
                        "Invalid header name '{key}': {e}"
                    ))
                })?;
            let header_value = HeaderValue::from_str(value).map_err(|e| {
                xai_tool_runtime::ToolError::invalid_arguments(format!(
                    "Invalid header value for '{key}': {e}"
                ))
            })?;
            headers.insert(header_name, header_value);
            Ok::<(), xai_tool_runtime::ToolError>(())
        })?;

        let http = crate::extra_ca::with_extra_root_certificates(reqwest::Client::builder())
            .timeout(std::time::Duration::from_secs(IMAGE_GEN_TIMEOUT_SECS))
            .read_timeout(std::time::Duration::from_secs(IMAGE_GEN_READ_TIMEOUT_SECS))
            .default_headers(headers)
            .build()
            .map_err(|e| {
                xai_tool_runtime::ToolError::invalid_arguments(format!(
                    "Failed to build HTTP client: {e}"
                ))
            })?;

        Ok(Self {
            http,
            base_url: base_url.clone(),
            edit_base_url,
            model,
            edit_model,
            provider: *provider,
            edit_provider: *edit_provider,
            writer: super::storage::SessionFileWriter::new(DEFAULT_IMAGE_DIR, "jpg"),
            api_key_provider,
            attribution_callback: None,
            tier_restricted: *tier_restricted,
        })
    }

    /// Whether the current user's tier (free / X Basic) is zero-limited on
    /// Imagine server-side. `image_gen` / `image_edit` use this to short-circuit
    /// with the SuperGrok upsell instead of issuing a doomed request.
    pub(crate) fn is_tier_restricted(&self) -> bool {
        self.tier_restricted
    }

    /// Wire a 401-attribution callback into this client. Idempotent;
    /// safe to call before or after the first request. Builder-style
    /// so `new()` callers that don't care can ignore it.
    pub fn with_attribution_callback(
        mut self,
        callback: Option<SharedAttributionCallback>,
    ) -> Self {
        self.attribution_callback = callback;
        self
    }

    pub(crate) async fn current_bearer(&self) -> Option<String> {
        crate::types::api_key_provider::resolve_bearer(self.api_key_provider.as_ref()).await
    }

    pub(crate) fn record_401_attribution(&self, consumer: ToolConsumer, sent_bearer: Option<&str>) {
        crate::attribution::emit_401(self.attribution_callback.as_ref(), consumer, sent_bearer);
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Base URL for `image_edit` (`/images/edits`).
    pub(crate) fn edit_base_url(&self) -> &str {
        &self.edit_base_url
    }

    /// Model slug for `image_edit`.
    pub(crate) fn edit_model(&self) -> &str {
        &self.edit_model
    }

    /// Wire shape both imagine surfaces use.
    pub(crate) fn provider(&self) -> MediaProvider {
        self.provider
    }

    /// Wire shape for `image_edit` only. Falls back to [`Self::provider`] at
    /// resolution time when the edit surface has no key of its own, so this is
    /// already the effective value.
    pub(crate) fn edit_provider(&self) -> MediaProvider {
        self.edit_provider
    }

    /// `true` when the configured endpoint is declared not to serve imagine;
    /// the tools return the remedy prose instead of a doomed request.
    pub(crate) fn is_unsupported_endpoint(&self) -> bool {
        self.provider().is_unsupported()
    }

    /// Same, for the edit surface (which has its own `provider` key).
    pub(crate) fn is_unsupported_edit_endpoint(&self) -> bool {
        self.edit_provider.is_unsupported()
    }

    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub(crate) fn writer(&self) -> &super::storage::SessionFileWriter {
        &self.writer
    }

    pub async fn generate(
        &self,
        prompt: &str,
        aspect_ratio: &str,
    ) -> Result<Vec<u8>, xai_tool_runtime::ToolError> {
        let url = format!("{}/images/generations", self.base_url.trim_end_matches('/'));

        let payload = generation_payload(&self.model, prompt, aspect_ratio, self.provider);

        // Capture the bearer once so the request and the 401-attribution
        // emit see the same value (even if the provider rotates between
        // the send and the response handling).
        let sent_bearer = self.current_bearer().await;
        let mut req = self.http.post(&url).json(&payload);
        if let Some(ref key) = sent_bearer {
            req = req.header(AUTHORIZATION, format!("Bearer {key}"));
        }

        let response = req.send().await.map_err(|e| {
            xai_tool_runtime::ToolError::invalid_arguments(format!(
                "Image generation API request failed: {e}"
            ))
        })?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            self.record_401_attribution(ToolConsumer::ImageGen, sent_bearer.as_deref());
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let truncated: String = body.chars().take(200).collect();
            tracing::warn!(http_status = %status, "Imagine API error: {truncated}");
            let detail = if status_is_missing_route(status) {
                format!(
                    "{} {truncated}",
                    missing_route_message(
                        MediaSurface::ImageGen,
                        &self.base_url,
                        status,
                        "/images/generations"
                    )
                )
            } else {
                format!("Image generation failed with HTTP {status}: {truncated}")
            };
            return Err(xai_tool_runtime::ToolError::new(
                xai_tool_runtime::ToolErrorKind::Custom,
                detail,
            )
            .with_details(serde_json::json!({"code": "http_failure", "status": status.as_u16()})));
        }

        let body = response.text().await.map_err(|e| {
            xai_tool_runtime::ToolError::invalid_arguments(format!(
                "Failed to read image generation response body: {e}"
            ))
        })?;

        decode_image_response(&self.http, &body, "image generation").await
    }
}

/// Imagine `/images/generations` payload.
///
/// `Auto`/`Xai` is the historical xAI body (`aspect_ratio` + `resolution`);
/// `OpenAi` swaps those for the OpenAI `size` field so a strict
/// OpenAI-compatible gateway is not handed unknown keys.
fn generation_payload(
    model: &str,
    prompt: &str,
    aspect_ratio: &str,
    provider: MediaProvider,
) -> serde_json::Value {
    if provider.is_openai_shape() {
        serde_json::json!({
            "model": model,
            "prompt": prompt,
            "n": 1,
            "size": openai_size_for_aspect_ratio(aspect_ratio),
            "response_format": "b64_json",
        })
    } else {
        serde_json::json!({
            "model": model,
            "prompt": prompt,
            "n": 1,
            "aspect_ratio": aspect_ratio,
            "resolution": "1k",
            "response_format": "b64_json",
        })
    }
}

/// Decode an imagine-family response body into image bytes.
///
/// Accepts the xAI and the OpenAI envelope (both are `{"data": [ … ]}`) and,
/// within an entry, either inline `b64_json` (with or without a `data:` URL
/// prefix) or a `url` — a `data:` URL is decoded locally, an `http(s)` URL is
/// fetched. A gateway that ignores `response_format=b64_json` therefore still
/// works instead of failing with "returned no image data".
pub(crate) async fn decode_image_response(
    http: &reqwest::Client,
    body: &str,
    surface: &str,
) -> Result<Vec<u8>, xai_tool_runtime::ToolError> {
    let resp_json: ImageGenResponse = serde_json::from_str(body).map_err(|e| {
        let preview: String = body.chars().take(500).collect();
        tracing::warn!("Imagine API returned unparseable body: {preview}");
        xai_tool_runtime::ToolError::invalid_arguments(format!(
            "Failed to parse {surface} response: {e} — body preview: {preview}"
        ))
    })?;

    let Some(payload) = resp_json.first_image() else {
        return Err(xai_tool_runtime::ToolError::invalid_arguments(format!(
            "{surface} returned no image data."
        )));
    };

    match payload {
        ImagePayload::B64(data) => decode_base64_image(&data),
        ImagePayload::Url(url) if url.starts_with("data:") => decode_base64_image(&url),
        ImagePayload::Url(url) => {
            let response = http.get(&url).send().await.map_err(|e| {
                xai_tool_runtime::ToolError::invalid_arguments(format!(
                    "{surface} returned an image URL that could not be fetched: {e}"
                ))
            })?;
            if !response.status().is_success() {
                return Err(xai_tool_runtime::ToolError::invalid_arguments(format!(
                    "{surface} returned an image URL that failed with HTTP {}",
                    response.status()
                )));
            }
            Ok(response
                .bytes()
                .await
                .map_err(|e| {
                    xai_tool_runtime::ToolError::invalid_arguments(format!(
                        "{surface} image download failed: {e}"
                    ))
                })?
                .to_vec())
        }
    }
}

/// Decode base64 image data, tolerating a `data:<mime>;base64,` prefix that
/// some OpenAI-compatible gateways leave on `b64_json`.
fn decode_base64_image(data: &str) -> Result<Vec<u8>, xai_tool_runtime::ToolError> {
    let trimmed = data.trim();
    let b64 = match trimmed.strip_prefix("data:") {
        Some(rest) => rest
            .split_once(',')
            .map(|(_, payload)| payload)
            .unwrap_or(rest),
        None => trimmed,
    };
    if b64.is_empty() {
        return Err(xai_tool_runtime::ToolError::invalid_arguments(
            "image generation returned no image data.",
        ));
    }
    base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| {
            xai_tool_runtime::ToolError::invalid_arguments(format!(
                "Failed to decode base64 image data: {e}"
            ))
        })
}

/// `Enabled` means credentials are present; each tool has its own gate.
#[derive(Debug, Clone, Default)]
pub enum ImageGenConfig {
    #[default]
    Disabled,
    Enabled {
        api_key: String,
        base_url: String,
        /// Base URL for `image_edit` (`/images/edits`). `None` = same as
        /// `base_url`. Set from `[tools.image_edit] base_url` /
        /// `GROK_IMAGE_EDIT_BASE_URL`.
        edit_base_url: Option<String>,
        extra_headers: indexmap::IndexMap<String, String>,
        image_gen_enabled: bool,
        image_edit_enabled: bool,
        /// Optional Imagine model override for `image_gen`. When `Some(non-empty)`,
        /// `image_gen` calls that model instead of the default quality model
        /// ([`XAI_IMAGINE_MODEL`]). Driven by the remote
        /// `image_gen_model_override` config flag, or by
        /// `[tools.image_gen] model` / `GROK_IMAGE_MODEL`, which win over it.
        model_override: Option<String>,
        /// Optional model override for `image_edit` only. `None` keeps
        /// [`super::image_edit::XAI_IMAGINE_EDIT_MODEL`].
        edit_model_override: Option<String>,
        /// Wire shape for `/images/*`. `Auto` (default) keeps the historical
        /// xAI payload and additionally accepts the OpenAI response envelope.
        provider: MediaProvider,
        /// Wire shape for `/images/edits` only. Set from
        /// `[tools.image_edit] provider` / `GROK_IMAGE_EDIT_PROVIDER`, falling
        /// back to `provider` when the edit surface has no key of its own — so
        /// one family-wide setting keeps working while an edit-only
        /// `provider = "unsupported"` silences just the edit surface (the
        /// remedy [`super::image_edit`]'s 404/405 message names).
        edit_provider: MediaProvider,
        /// `true` when the user is on a tier the Imagine server zero-limits
        /// (free / X Basic). The tools stay advertised to the model, but
        /// `image_gen` / `image_edit` short-circuit at call time with the
        /// SuperGrok upsell prose instead of a doomed request. Set by the
        /// host from the subscription tier; always `false` for team /
        /// API-key / workspace callers.
        tier_restricted: bool,
    },
}

impl ImageGenConfig {
    /// Credentials present — required to construct any of the clients.
    pub fn has_credentials(&self) -> bool {
        matches!(self, Self::Enabled { .. })
    }

    pub fn image_gen_enabled(&self) -> bool {
        matches!(
            self,
            Self::Enabled {
                image_gen_enabled: true,
                ..
            }
        )
    }

    pub fn image_edit_enabled(&self) -> bool {
        matches!(
            self,
            Self::Enabled {
                image_edit_enabled: true,
                ..
            }
        )
    }

    /// The configured `image_gen` model override, if any. `None` means the
    /// default quality model ([`XAI_IMAGINE_MODEL`]) is used.
    pub fn model_override(&self) -> Option<&str> {
        match self {
            Self::Enabled { model_override, .. } => {
                model_override.as_deref().filter(|m| !m.trim().is_empty())
            }
            Self::Disabled => None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ImageGenInput {
    #[schemars(description = "Text description of the image to generate.")]
    pub prompt: String,

    #[serde(default = "default_aspect_ratio")]
    #[schemars(
        description = "Aspect ratio of the generated image, decide it based on the user's request. Defaults to 'auto'. 1:1 for square (icons, profiles), 16:9 for wide (landscapes, cinematic), 9:16 for tall (phone wallpapers, stories), 3:2 for horizontal photos, 2:3 for vertical (portraits, posters)."
    )]
    pub aspect_ratio: String,
}

fn default_aspect_ratio() -> String {
    "auto".to_owned()
}

#[derive(Debug, serde::Deserialize)]
pub struct ImageGenResponse {
    #[serde(default)]
    data: Vec<ImageGenData>,
}

/// One image out of a response entry: either inline base64 or a URL (which may
/// itself be a `data:` URL). The OpenAI and xAI envelopes both use these keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImagePayload {
    B64(String),
    Url(String),
}

impl ImageGenResponse {
    /// Inline base64 or URL of the first usable image, as a string. Kept for
    /// callers that only need "what came back".
    pub fn b64_data(&self) -> Option<String> {
        match self.first_image()? {
            ImagePayload::B64(data) => Some(data),
            ImagePayload::Url(url) => Some(url),
        }
    }

    /// First usable image in the response, preferring inline base64 over a
    /// URL (no extra round trip).
    pub fn first_image(&self) -> Option<ImagePayload> {
        let entry = self.data.first()?;
        if let Some(b64) = entry.b64_json.as_deref().filter(|s| !s.trim().is_empty()) {
            return Some(ImagePayload::B64(b64.to_owned()));
        }
        entry
            .url
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|url| ImagePayload::Url(url.to_owned()))
    }
}

#[derive(Debug, serde::Deserialize)]
struct ImageGenData {
    b64_json: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Default)]
pub struct ImageGenTool;

impl crate::types::tool_metadata::ToolMetadata for ImageGenTool {
    fn kind(&self) -> ToolKind {
        ToolKind::ImageGen
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Generate a new image from a text description using Imagine; returns the saved image's absolute path. When telling the user where it was saved, refer to it by its short session-relative path (e.g. `images/1.jpg`) rather than the absolute path, so it renders as a clickable link that opens the image. To produce multiple images, emit multiple tool calls with distinct prompts."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for ImageGenTool {
    type Args = ImageGenInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("image_gen").expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "image_gen",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: false,
            tool_scope: Some(xai_tool_protocol::ToolScope::Write),
            ..Default::default()
        }
    }

    #[tracing::instrument(
        name = "tool.image_gen",
        skip_all,
        fields(prompt_len = input.prompt.len(), aspect_ratio = %input.aspect_ratio)
    )]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: ImageGenInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;
        let resources = shared_resources(&ctx)?;

        let client = {
            let res = resources.lock().await;
            res.require::<ImageGenClient>()?.clone()
        };

        // Free / X Basic users are zero-limited on Imagine server-side; return
        // the upsell prose instead of a doomed request (the tool stays
        // advertised so the model can surface the nudge in-conversation).
        if client.is_tier_restricted() {
            return Ok(ToolOutput::Text(TIER_RESTRICTED_UPSELL.into()));
        }

        // The configured endpoint is declared not to serve imagine (`provider
        // = "unsupported"`): say which key moves it instead of issuing a call
        // that 404s.
        if client.is_unsupported_endpoint() {
            return Ok(ToolOutput::Text(
                unsupported_surface_message(MediaSurface::ImageGen, client.base_url()).into(),
            ));
        }

        let image_bytes = client.generate(&input.prompt, &input.aspect_ratio).await?;

        let session_folder = {
            let res = resources.lock().await;
            res.require::<SessionFolder>()?.0.clone()
        };

        let absolute_path = client
            .writer
            .save(&session_folder, &image_bytes, None)
            .await
            .map_err(|e| xai_tool_runtime::ToolError::invalid_arguments(e.to_string()))?;

        tracing::info!(
            path = %absolute_path.display(),
            bytes = image_bytes.len(),
            "image saved to disk"
        );

        Ok(ToolOutput::ImageGen(MediaGenOutput::new(absolute_path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tool_metadata::test_ctx_with_call_id;

    /// Config with every new field at its default: `[tools.*]` unset.
    fn cfg(base_url: &str) -> ImageGenConfig {
        ImageGenConfig::Enabled {
            api_key: "k".into(),
            base_url: base_url.into(),
            edit_base_url: None,
            extra_headers: indexmap::IndexMap::new(),
            image_gen_enabled: true,
            image_edit_enabled: true,
            model_override: None,
            edit_model_override: None,
            provider: MediaProvider::Auto,
            edit_provider: MediaProvider::Auto,
            tier_restricted: false,
        }
    }

    fn resources_with_client(client: ImageGenClient) -> crate::types::resources::SharedResources {
        let mut resources = crate::types::resources::Resources::new();
        resources.insert(client);
        resources.into_shared()
    }

    #[test]
    fn tool_name_and_description() {
        let tool = ImageGenTool;
        assert_eq!(xai_tool_runtime::Tool::id(&tool).as_str(), "image_gen");
        assert!(
            crate::types::tool_metadata::ToolMetadata::description_template(&tool)
                .contains("Generate a new image from a text description")
        );
    }

    #[test]
    fn default_aspect_ratio_is_auto() {
        let input: ImageGenInput = serde_json::from_str(r#"{"prompt": "test"}"#).unwrap();
        assert_eq!(input.aspect_ratio, "auto");
    }

    #[test]
    fn per_tool_gates_are_independent() {
        let cfg = ImageGenConfig::Enabled {
            api_key: "k".into(),
            base_url: "https://api.x.ai/v1".into(),
            edit_base_url: None,
            extra_headers: indexmap::IndexMap::new(),
            image_gen_enabled: false,
            image_edit_enabled: true,
            model_override: Some("grok-imagine-image".into()),
            edit_model_override: None,
            provider: MediaProvider::Auto,
            edit_provider: MediaProvider::Auto,
            tier_restricted: false,
        };
        assert!(cfg.has_credentials());
        assert!(!cfg.image_gen_enabled());
        assert!(cfg.image_edit_enabled());
        assert_eq!(cfg.model_override(), Some("grok-imagine-image"));

        assert!(!ImageGenConfig::Disabled.has_credentials());
    }

    #[test]
    fn client_selects_model_from_override() {
        let mk = |model_override: Option<&str>| ImageGenConfig::Enabled {
            api_key: "k".into(),
            base_url: "https://api.x.ai/v1".into(),
            edit_base_url: None,
            extra_headers: indexmap::IndexMap::new(),
            image_gen_enabled: true,
            image_edit_enabled: true,
            model_override: model_override.map(String::from),
            edit_model_override: None,
            provider: MediaProvider::Auto,
            edit_provider: MediaProvider::Auto,
            tier_restricted: false,
        };
        // No override → default quality model.
        assert_eq!(
            ImageGenClient::new(&mk(None), None).unwrap().model,
            XAI_IMAGINE_MODEL
        );
        // Empty override → treated as no override.
        assert_eq!(
            ImageGenClient::new(&mk(Some("")), None).unwrap().model,
            XAI_IMAGINE_MODEL
        );
        // Override → that exact model slug.
        assert_eq!(
            ImageGenClient::new(&mk(Some("grok-imagine-image")), None)
                .unwrap()
                .model,
            "grok-imagine-image"
        );
    }

    /// The edit surface follows `edit_base_url`/`edit_model_override` and falls
    /// back to the imagine base URL and the edit default model.
    #[test]
    fn client_resolves_edit_surface_independently() {
        let client = ImageGenClient::new(&cfg("https://api.x.ai/v1"), None).unwrap();
        assert_eq!(client.base_url(), "https://api.x.ai/v1");
        assert_eq!(client.edit_base_url(), "https://api.x.ai/v1");
        assert_eq!(
            client.edit_model(),
            crate::implementations::grok_build::image_edit::XAI_IMAGINE_EDIT_MODEL
        );

        let mut config = cfg("https://imagine.example/v1");
        if let ImageGenConfig::Enabled {
            edit_base_url,
            edit_model_override,
            ..
        } = &mut config
        {
            *edit_base_url = Some("https://edits.example/v1".into());
            *edit_model_override = Some("gpt-image-1-edit".into());
        }
        let client = ImageGenClient::new(&config, None).unwrap();
        assert_eq!(client.base_url(), "https://imagine.example/v1");
        assert_eq!(client.edit_base_url(), "https://edits.example/v1");
        assert_eq!(client.edit_model(), "gpt-image-1-edit");
    }

    #[tokio::test]
    async fn errors_when_client_missing() {
        let tool = ImageGenTool;
        let resources = crate::types::resources::Resources::new();
        let result = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx_with_call_id(resources.into_shared(), "test-call"),
            ImageGenInput {
                prompt: "a test image".into(),
                aspect_ratio: "auto".into(),
            },
        )
        .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("missing required resource"),
            "Expected MissingResource error, got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn tier_restricted_short_circuits_with_upsell() {
        // A free / X Basic user's image_gen call returns the SuperGrok upsell
        // prose as a normal result (no HTTP, no error card) so the model can
        // relay it. Only the client is inserted — the short-circuit returns
        // before any other resource (e.g. SessionFolder) is required.
        let mut config = cfg("https://api.x.ai/v1");
        if let ImageGenConfig::Enabled {
            tier_restricted, ..
        } = &mut config
        {
            *tier_restricted = true;
        }
        let resources = resources_with_client(ImageGenClient::new(&config, None).unwrap());

        let result = xai_tool_runtime::Tool::run(
            &ImageGenTool,
            test_ctx_with_call_id(resources, "test-call"),
            ImageGenInput {
                prompt: "a cat".into(),
                aspect_ratio: "auto".into(),
            },
        )
        .await
        .expect("tier-restricted call must succeed with upsell prose");

        match result {
            ToolOutput::Text(t) => {
                assert!(t.text.contains("SuperGrok"), "got: {}", t.text);
                assert!(t.text.contains("supergrok?referrer=grok-build"));
            }
            other => panic!("expected Text upsell, got {other:?}"),
        }
    }

    /// `provider = "unsupported"` short-circuits with the key to set, before
    /// any HTTP call.
    #[tokio::test]
    async fn unsupported_provider_short_circuits_with_remedy() {
        let mut config = cfg("https://gateway.example/v1");
        if let ImageGenConfig::Enabled { provider, .. } = &mut config {
            *provider = MediaProvider::Unsupported;
        }
        let resources = resources_with_client(ImageGenClient::new(&config, None).unwrap());

        let result = xai_tool_runtime::Tool::run(
            &ImageGenTool,
            test_ctx_with_call_id(resources, "test-call"),
            ImageGenInput {
                prompt: "a cat".into(),
                aspect_ratio: "auto".into(),
            },
        )
        .await
        .expect("unsupported endpoint must return prose, not an error");

        match result {
            ToolOutput::Text(t) => {
                assert!(t.text.contains("GROK_IMAGE_BASE_URL"), "got: {}", t.text);
                assert!(
                    t.text.contains("[tools.image_gen] base_url"),
                    "got: {}",
                    t.text
                );
                assert!(t.text.contains("Do not retry"), "got: {}", t.text);
            }
            other => panic!("expected Text remedy, got {other:?}"),
        }
    }

    // ── wire shape ───────────────────────────────────────────────────

    /// Auto/xai payload is byte-identical to the historical xAI body.
    #[test]
    fn auto_payload_keeps_xai_fields() {
        let payload = generation_payload("m", "a cat", "16:9", MediaProvider::Auto);
        assert_eq!(payload["model"], "m");
        assert_eq!(payload["prompt"], "a cat");
        assert_eq!(payload["n"], 1);
        assert_eq!(payload["aspect_ratio"], "16:9");
        assert_eq!(payload["resolution"], "1k");
        assert_eq!(payload["response_format"], "b64_json");
        assert!(payload.get("size").is_none());
        assert_eq!(
            payload,
            generation_payload("m", "a cat", "16:9", MediaProvider::Xai)
        );
    }

    /// The OpenAI shape swaps `aspect_ratio`/`resolution` for `size`.
    #[test]
    fn openai_payload_uses_size_not_xai_fields() {
        let payload = generation_payload("m", "a cat", "16:9", MediaProvider::OpenAi);
        assert_eq!(payload["size"], "1536x1024");
        assert!(payload.get("aspect_ratio").is_none());
        assert!(payload.get("resolution").is_none());
        assert_eq!(payload["response_format"], "b64_json");
    }

    #[test]
    fn response_prefers_b64_then_url() {
        let resp: ImageGenResponse =
            serde_json::from_str(r#"{"data":[{"b64_json":"AAAA","url":"https://x/y.png"}]}"#)
                .unwrap();
        assert_eq!(resp.first_image(), Some(ImagePayload::B64("AAAA".into())));

        let resp: ImageGenResponse =
            serde_json::from_str(r#"{"data":[{"url":"https://x/y.png"}]}"#).unwrap();
        assert_eq!(
            resp.first_image(),
            Some(ImagePayload::Url("https://x/y.png".into()))
        );

        let resp: ImageGenResponse = serde_json::from_str(r#"{"data":[]}"#).unwrap();
        assert_eq!(resp.first_image(), None);
    }

    #[test]
    fn base64_decode_tolerates_data_url_prefix() {
        let bytes = decode_base64_image("data:image/png;base64,QUJD").unwrap();
        assert_eq!(bytes, b"ABC");
        let bytes = decode_base64_image("QUJD").unwrap();
        assert_eq!(bytes, b"ABC");
        assert!(decode_base64_image("data:image/png;base64,").is_err());
        assert!(decode_base64_image("not base64!").is_err());
    }

    // ── mock-server contract ─────────────────────────────────────────

    /// End-to-end against an OpenAI-shaped gateway: the request carries the
    /// OpenAI fields and the response envelope is decoded without a
    /// translation layer.
    #[tokio::test]
    async fn openai_shape_gateway_roundtrip() {
        use wiremock::matchers::{body_partial_json, header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/images/generations"))
            .and(header("Authorization", "Bearer gw-key"))
            .and(body_partial_json(serde_json::json!({
                "model": "gpt-image-1",
                "n": 1,
                "size": "1536x1024",
                "response_format": "b64_json",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "created": 1,
                "data": [{"b64_json": "QUJD"}],
            })))
            .expect(1)
            .mount(&server)
            .await;

        let mut config = cfg(&server.uri());
        if let ImageGenConfig::Enabled {
            api_key,
            model_override,
            provider,
            ..
        } = &mut config
        {
            *api_key = "gw-key".into();
            *model_override = Some("gpt-image-1".into());
            *provider = MediaProvider::OpenAi;
        }
        let client = ImageGenClient::new(&config, None).unwrap();
        let bytes = client.generate("a cat", "16:9").await.expect("roundtrip");
        assert_eq!(bytes, b"ABC");
    }

    /// A gateway that ignores `response_format` and answers with a URL still
    /// yields bytes.
    #[tokio::test]
    async fn url_only_response_is_fetched() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/images/generations"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"url": format!("{}/blob.png", server.uri())}],
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/blob.png"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"PNGDATA".to_vec()))
            .expect(1)
            .mount(&server)
            .await;

        let client = ImageGenClient::new(&cfg(&server.uri()), None).unwrap();
        let bytes = client.generate("a cat", "auto").await.expect("url fetch");
        assert_eq!(bytes, b"PNGDATA");
    }

    /// A 404 names the key to set instead of returning a bare status.
    #[tokio::test]
    async fn missing_route_reports_remedy() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/images/generations"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
            .mount(&server)
            .await;

        let client = ImageGenClient::new(&cfg(&server.uri()), None).unwrap();
        let err = client
            .generate("a cat", "auto")
            .await
            .expect_err("404 must fail");
        let msg = err.to_string();
        assert!(msg.contains("HTTP 404"), "got: {msg}");
        assert!(msg.contains("GROK_IMAGE_BASE_URL"), "got: {msg}");
        assert!(msg.contains("provider = \"unsupported\""), "got: {msg}");
    }

    /// A non-route failure keeps the historical wording.
    #[tokio::test]
    async fn server_error_keeps_plain_message() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/images/generations"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;

        let client = ImageGenClient::new(&cfg(&server.uri()), None).unwrap();
        let msg = client
            .generate("a cat", "auto")
            .await
            .expect_err("500 must fail")
            .to_string();
        assert!(
            msg.contains("Image generation failed with HTTP 500"),
            "got: {msg}"
        );
        assert!(msg.contains("boom"), "got: {msg}");
    }
}
