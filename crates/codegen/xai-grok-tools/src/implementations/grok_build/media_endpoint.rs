//! Per-surface media endpoint resolution: base URL, model, and wire provider.
//!
//! `image_gen`, `image_edit`, and the video tools each talk to a *different*
//! API surface, but all of them used to inherit `[endpoints].xai_api_base_url`
//! — the chat endpoint. That is correct when everything is xAI and wrong the
//! moment chat points at a local or third-party gateway that does not serve
//! `/images/*` or `/videos/*`.
//!
//! Resolution reads the **raw config table** (the same way
//! `xai-grok-voice`'s `VoiceConfig::from_config_table` does) instead of a typed
//! `[tools]` struct, so the keys need no schema churn. The caller owns the env
//! and table lookups; this module owns the precedence, the wire-shape choice,
//! and the remedy text:
//!
//! ```text
//! GROK_IMAGE_BASE_URL      > [tools.image_gen].base_url  > endpoints.xai_api_base_url
//! GROK_IMAGE_MODEL         > [tools.image_gen].model     > remote image_gen_model_override > default
//! GROK_IMAGE_EDIT_BASE_URL > [tools.image_edit].base_url > the resolved image base URL
//! GROK_IMAGE_EDIT_MODEL    > [tools.image_edit].model    > default
//! GROK_IMAGE_PROVIDER      > [tools.image_gen].provider  > auto
//! GROK_IMAGE_EDIT_PROVIDER > [tools.image_edit].provider > the image provider
//! GROK_VIDEO_BASE_URL      > [tools.video_gen].base_url  > endpoints.xai_api_base_url
//! GROK_VIDEO_MODEL         > [tools.video_gen].model     > default
//! GROK_VIDEO_PROVIDER      > [tools.video_gen].provider  > auto
//! ```
//!
//! `image_edit` is the only surface with an inherited *provider*: it follows
//! the image-generation value when it has no key of its own, so a single
//! family-wide `[tools.image_gen] provider` keeps working while an edit-only
//! `provider = "unsupported"` still silences just that surface (the remedy the
//! 404/405 message names — see [`missing_route_message`]).
//!
//! The env tier wins over the config tier for these keys (matching the other
//! Phase-4 surface resolvers); with every new key unset the resolved values are
//! exactly what the tools used before this module existed.

use reqwest::StatusCode;

/// A media surface with its own base URL, model, and wire provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaSurface {
    ImageGen,
    ImageEdit,
    VideoGen,
}

impl MediaSurface {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ImageGen => "image_gen",
            Self::ImageEdit => "image_edit",
            Self::VideoGen => "video_gen",
        }
    }

    /// `[tools.<table>]` section that configures this surface.
    pub fn config_table(self) -> &'static str {
        self.as_str()
    }

    pub fn base_url_env(self) -> &'static str {
        match self {
            Self::ImageGen => "GROK_IMAGE_BASE_URL",
            Self::ImageEdit => "GROK_IMAGE_EDIT_BASE_URL",
            Self::VideoGen => "GROK_VIDEO_BASE_URL",
        }
    }

    pub fn model_env(self) -> &'static str {
        match self {
            Self::ImageGen => "GROK_IMAGE_MODEL",
            Self::ImageEdit => "GROK_IMAGE_EDIT_MODEL",
            Self::VideoGen => "GROK_VIDEO_MODEL",
        }
    }

    pub fn provider_env(self) -> &'static str {
        match self {
            Self::ImageGen => "GROK_IMAGE_PROVIDER",
            Self::ImageEdit => "GROK_IMAGE_EDIT_PROVIDER",
            Self::VideoGen => "GROK_VIDEO_PROVIDER",
        }
    }

    /// Surface whose *provider* this one inherits when it has no key of its
    /// own. Only `image_edit` inherits (from `image_gen`), so one family-wide
    /// setting still covers both imagine surfaces; `None` means "no
    /// inheritance, the default applies".
    pub fn provider_fallback(self) -> Option<Self> {
        match self {
            Self::ImageEdit => Some(Self::ImageGen),
            Self::ImageGen | Self::VideoGen => None,
        }
    }

    /// Human label used in failure messages.
    pub fn label(self) -> &'static str {
        match self {
            Self::ImageGen => "image generation",
            Self::ImageEdit => "image editing",
            Self::VideoGen => "video generation",
        }
    }

    /// API paths the endpoint has to serve for this surface to work.
    pub fn paths(self) -> &'static str {
        match self {
            Self::ImageGen => "/images/generations",
            Self::ImageEdit => "/images/edits",
            Self::VideoGen => "/videos/generations and /videos/{id}",
        }
    }
}

/// Which request/response wire a surface speaks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MediaProvider {
    /// Default: send the xAI-shaped request and accept both the xAI and the
    /// OpenAI response envelope. Works against xAI and against gateways that
    /// tolerate the extra `aspect_ratio` / `resolution` fields.
    #[default]
    Auto,
    /// Explicit alias of [`Self::Auto`].
    Xai,
    /// Send the OpenAI request shape (`size` instead of `aspect_ratio` /
    /// `resolution`) and accept the OpenAI response envelope. For the video
    /// tools this is the same wire as `auto` — the OpenAI video API is not
    /// implemented.
    OpenAi,
    /// The endpoint does not serve this surface. The tool short-circuits with a
    /// remedy instead of issuing a doomed request.
    Unsupported,
}

impl MediaProvider {
    /// Parse a `provider` value. Case-insensitive; `None` for an unknown value
    /// so the caller can warn and fall back to the default instead of failing
    /// the session.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" => None,
            "auto" => Some(Self::Auto),
            "xai" => Some(Self::Xai),
            "openai" | "open_ai" | "open-ai" => Some(Self::OpenAi),
            "unsupported" | "none" | "off" => Some(Self::Unsupported),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Xai => "xai",
            Self::OpenAi => "openai",
            Self::Unsupported => "unsupported",
        }
    }

    /// Whether this surface should be skipped without an HTTP call.
    pub fn is_unsupported(self) -> bool {
        matches!(self, Self::Unsupported)
    }

    /// Whether requests must use the strict OpenAI request shape.
    pub fn is_openai_shape(self) -> bool {
        matches!(self, Self::OpenAi)
    }
}

/// Where a resolved value came from (for logs and diagnostics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointSource {
    Env,
    Config,
    Default,
}

impl EndpointSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Env => "env",
            Self::Config => "config",
            Self::Default => "default",
        }
    }
}

/// A resolved value plus its origin. Never empty: an empty env/config value
/// falls through to the next tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedValue {
    pub value: String,
    pub source: EndpointSource,
}

/// First non-empty of `env` > `config` > `fallback`.
///
/// The winning value is trimmed and has trailing slashes stripped so callers
/// can always append `/{path}`; a blank tier falls through instead of pinning
/// an empty base URL.
pub fn resolve_value(env: Option<&str>, config: Option<&str>, fallback: &str) -> ResolvedValue {
    if let Some(value) = non_empty(env) {
        return ResolvedValue {
            value: normalize_base(value),
            source: EndpointSource::Env,
        };
    }
    if let Some(value) = non_empty(config) {
        return ResolvedValue {
            value: normalize_base(value),
            source: EndpointSource::Config,
        };
    }
    ResolvedValue {
        value: normalize_base(fallback),
        source: EndpointSource::Default,
    }
}

fn normalize_base(value: &str) -> String {
    value.trim().trim_end_matches('/').to_owned()
}

/// First non-empty of `env` > `config` > `existing`; `None` when all are unset
/// (the client then applies its own default model).
pub fn resolve_optional_value(
    env: Option<&str>,
    config: Option<&str>,
    existing: Option<&str>,
) -> Option<String> {
    non_empty(env)
        .or_else(|| non_empty(config))
        .or_else(|| non_empty(existing))
        .map(str::to_owned)
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

/// Whether `status` means "this endpoint does not serve the path" rather than a
/// request-level error: 404 (no route) or 405 (route exists, method not
/// allowed). 501 is included because a proxy that does not implement a surface
/// reports it that way.
pub fn status_is_missing_route(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED | StatusCode::NOT_IMPLEMENTED
    )
}

/// OpenAI image `size` for an xAI `aspect_ratio`. Numeric `W:H` maps to the
/// nearest OpenAI size bucket; anything else (including `auto`) stays `auto`.
pub fn openai_size_for_aspect_ratio(aspect_ratio: &str) -> &'static str {
    let trimmed = aspect_ratio.trim();
    let Some((w, h)) = trimmed.split_once(':') else {
        return "auto";
    };
    let (Ok(w), Ok(h)) = (w.trim().parse::<f64>(), h.trim().parse::<f64>()) else {
        return "auto";
    };
    if w <= 0.0 || h <= 0.0 {
        return "auto";
    }
    match w.total_cmp(&h) {
        std::cmp::Ordering::Equal => "1024x1024",
        std::cmp::Ordering::Greater => "1536x1024",
        std::cmp::Ordering::Less => "1024x1536",
    }
}

/// Remedy returned by the tools when `provider = "unsupported"` (or an explicit
/// "this endpoint does not serve it"): names the key to set and asks the model
/// not to retry, mirroring the tier-upsell prose.
pub fn unsupported_surface_message(surface: MediaSurface, base_url: &str) -> String {
    format!(
        "{label} is not served by the configured endpoint ({base_url}). Set {env} (or \
         [tools.{table}] base_url) to an endpoint that serves {paths}, then drop \
         [tools.{table}] provider = \"unsupported\". Do not retry this tool.",
        label = surface.label(),
        env = surface.base_url_env(),
        table = surface.config_table(),
        paths = surface.paths(),
    )
}

/// Error text for a 404/405/501 from a media endpoint: says which endpoint
/// refused, which keys move it (base URL *and* model — a gateway usually 404s
/// an unknown model too), and how to silence the surface instead.
pub fn missing_route_message(
    surface: MediaSurface,
    base_url: &str,
    status: StatusCode,
    path: &str,
) -> String {
    format!(
        "{label} failed with HTTP {status}: {base}{path} does not serve the {label} API. \
         Point {env} (or [tools.{table}] base_url) at a gateway that serves {paths}, set \
         {model_env} (or [tools.{table}] model) to a model it serves, or set \
         [tools.{table}] provider = \"unsupported\" to silence this surface.",
        label = surface.label(),
        base = base_url.trim_end_matches('/'),
        env = surface.base_url_env(),
        model_env = surface.model_env(),
        table = surface.config_table(),
        paths = surface.paths(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_beats_config_beats_fallback() {
        let resolved = resolve_value(
            Some("https://env.example/v1"),
            Some("https://cfg.example"),
            "https://api.x.ai/v1",
        );
        assert_eq!(resolved.value, "https://env.example/v1");
        assert_eq!(resolved.source, EndpointSource::Env);

        let resolved = resolve_value(None, Some("https://cfg.example"), "https://api.x.ai/v1");
        assert_eq!(resolved.value, "https://cfg.example");
        assert_eq!(resolved.source, EndpointSource::Config);

        let resolved = resolve_value(None, None, "https://api.x.ai/v1");
        assert_eq!(resolved.value, "https://api.x.ai/v1");
        assert_eq!(resolved.source, EndpointSource::Default);
    }

    #[test]
    fn blank_env_and_config_fall_through() {
        let resolved = resolve_value(Some("   "), Some(""), "https://api.x.ai/v1");
        assert_eq!(resolved.value, "https://api.x.ai/v1");
        assert_eq!(resolved.source, EndpointSource::Default);
    }

    #[test]
    fn resolved_values_are_trimmed_and_slash_normalized() {
        // Every tier normalizes: callers append `/{path}` without re-checking.
        let resolved = resolve_value(Some("  https://env.example/v1/  "), None, "unused");
        assert_eq!(resolved.value, "https://env.example/v1");
        let resolved = resolve_value(None, Some("https://cfg.example/"), "unused");
        assert_eq!(resolved.value, "https://cfg.example");
        let resolved = resolve_value(None, None, "https://api.x.ai/v1/");
        assert_eq!(resolved.value, "https://api.x.ai/v1");
    }

    #[test]
    fn optional_value_prefers_new_tiers_over_existing_override() {
        assert_eq!(
            resolve_optional_value(Some("env-model"), Some("cfg-model"), Some("remote-model")),
            Some("env-model".to_owned())
        );
        assert_eq!(
            resolve_optional_value(None, Some("cfg-model"), Some("remote-model")),
            Some("cfg-model".to_owned())
        );
        // The pre-existing override still wins over the built-in default.
        assert_eq!(
            resolve_optional_value(None, None, Some("remote-model")),
            Some("remote-model".to_owned())
        );
        assert_eq!(resolve_optional_value(None, None, None), None);
        assert_eq!(
            resolve_optional_value(Some("  "), Some(""), Some("")),
            None,
            "blank values must not pin an empty model"
        );
    }

    #[test]
    fn provider_parses_known_values_and_rejects_typos() {
        assert_eq!(MediaProvider::parse("auto"), Some(MediaProvider::Auto));
        assert_eq!(MediaProvider::parse("XAI"), Some(MediaProvider::Xai));
        assert_eq!(
            MediaProvider::parse(" openai "),
            Some(MediaProvider::OpenAi)
        );
        assert_eq!(
            MediaProvider::parse("unsupported"),
            Some(MediaProvider::Unsupported)
        );
        assert_eq!(MediaProvider::parse("openaii"), None);
        assert_eq!(MediaProvider::parse(""), None);
        assert_eq!(MediaProvider::default(), MediaProvider::Auto);
        assert!(MediaProvider::Unsupported.is_unsupported());
        assert!(MediaProvider::OpenAi.is_openai_shape());
    }

    #[test]
    fn missing_route_statuses_are_404_405_and_501() {
        assert!(status_is_missing_route(StatusCode::NOT_FOUND));
        assert!(status_is_missing_route(StatusCode::METHOD_NOT_ALLOWED));
        assert!(status_is_missing_route(StatusCode::NOT_IMPLEMENTED));
        assert!(!status_is_missing_route(StatusCode::BAD_REQUEST));
        assert!(!status_is_missing_route(StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn openai_size_maps_orientation_not_exact_ratio() {
        assert_eq!(openai_size_for_aspect_ratio("auto"), "auto");
        assert_eq!(openai_size_for_aspect_ratio(""), "auto");
        assert_eq!(openai_size_for_aspect_ratio("square-ish"), "auto");
        assert_eq!(openai_size_for_aspect_ratio("1:1"), "1024x1024");
        assert_eq!(openai_size_for_aspect_ratio("16:9"), "1536x1024");
        assert_eq!(openai_size_for_aspect_ratio("3:2"), "1536x1024");
        assert_eq!(openai_size_for_aspect_ratio("9:16"), "1024x1536");
        assert_eq!(openai_size_for_aspect_ratio("2:3"), "1024x1536");
        assert_eq!(openai_size_for_aspect_ratio("0:5"), "auto");
    }

    #[test]
    fn unsupported_message_names_the_env_and_config_key() {
        let msg = unsupported_surface_message(MediaSurface::VideoGen, "https://gw.example/v1");
        assert!(msg.contains("https://gw.example/v1"), "got: {msg}");
        assert!(msg.contains("GROK_VIDEO_BASE_URL"), "got: {msg}");
        assert!(msg.contains("[tools.video_gen] base_url"), "got: {msg}");
        assert!(msg.contains("Do not retry"), "got: {msg}");
    }

    #[test]
    fn missing_route_message_names_the_remedy_and_the_path() {
        let msg = missing_route_message(
            MediaSurface::ImageEdit,
            "https://gw.example/v1/",
            StatusCode::NOT_FOUND,
            "/images/edits",
        );
        assert!(msg.contains("HTTP 404"), "got: {msg}");
        assert!(
            msg.contains("https://gw.example/v1/images/edits"),
            "got: {msg}"
        );
        assert!(msg.contains("GROK_IMAGE_EDIT_BASE_URL"), "got: {msg}");
        assert!(msg.contains("[tools.image_edit] base_url"), "got: {msg}");
        assert!(msg.contains("GROK_IMAGE_EDIT_MODEL"), "got: {msg}");
        assert!(msg.contains("provider = \"unsupported\""), "got: {msg}");
    }

    #[test]
    fn every_surface_has_distinct_env_keys() {
        let surfaces = [
            MediaSurface::ImageGen,
            MediaSurface::ImageEdit,
            MediaSurface::VideoGen,
        ];
        for surface in surfaces {
            assert!(surface.base_url_env().starts_with("GROK_"));
            assert!(surface.model_env().starts_with("GROK_"));
            assert!(surface.provider_env().starts_with("GROK_"));
            assert!(!surface.paths().is_empty());
        }
        assert_ne!(
            MediaSurface::ImageGen.base_url_env(),
            MediaSurface::ImageEdit.base_url_env()
        );
        // The edit surface has its own provider key: the remedy names it, so it
        // has to be a key that is actually read.
        assert_eq!(
            MediaSurface::ImageEdit.provider_env(),
            "GROK_IMAGE_EDIT_PROVIDER"
        );
        assert_ne!(
            MediaSurface::ImageGen.provider_env(),
            MediaSurface::ImageEdit.provider_env()
        );
    }

    #[test]
    fn only_image_edit_inherits_a_provider() {
        assert_eq!(
            MediaSurface::ImageEdit.provider_fallback(),
            Some(MediaSurface::ImageGen)
        );
        assert_eq!(MediaSurface::ImageGen.provider_fallback(), None);
        assert_eq!(MediaSurface::VideoGen.provider_fallback(), None);
    }
}
