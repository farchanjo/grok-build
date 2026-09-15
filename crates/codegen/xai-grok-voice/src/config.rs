use serde::{Deserialize, Serialize};

use crate::error::VoiceError;

/// Default STT capture rate (Hz). Shared with the `__mic-capture` helper's
/// argv default so parent and child agree when `--rate` is omitted.
pub const DEFAULT_SAMPLE_RATE: u32 = 16_000;

/// Env override for the STT endpoint: `GROK_VOICE_BASE_URL`.
///
/// Voice is its own surface: an endpoint that serves chat (or even the
/// OpenAI-compatible `/images/*` routes) does not necessarily serve the xAI
/// `/stt` WebSocket. This key points STT somewhere else without moving chat.
/// It outranks `[voice].api_base` so a per-run override works.
pub const VOICE_BASE_URL_ENV: &str = "GROK_VOICE_BASE_URL";

/// Voice settings for the STT transport.
///
/// Prefer **https** `api_base` (same shape as chat). [`Self::stt_ws_url`] derives
/// `wss://`. `api_base` resolves as `GROK_VOICE_BASE_URL` > `[voice].api_base` >
/// `[endpoints].xai_api_base_url`, so an enterprise proxy needs no second knob
/// and a per-run override needs no edit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VoiceConfig {
    /// HTTPS API root (or bare host). Bases may end in `/v1` or `/xai/v1`; the
    /// default STT path de-duplicates a leading `v1/` so both become `…/v1/stt`.
    pub api_base: String,
    pub stt_ws_path: String,
    /// Preferred STT language (catalog code or `"auto"`). Resolved via
    /// [`crate::language_for_api`] at connect time.
    pub language: String,
    pub sample_rate: u32,
    pub stt_endpointing_ms: u32,
    pub stt_interim_results: bool,

    /// Pager-stamped request identity; not user config.
    #[serde(skip)]
    pub client_identifier: String,
    #[serde(skip)]
    pub user_agent: String,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            api_base: "https://api.x.ai".into(),
            stt_ws_path: "/v1/stt".into(),
            language: "en".into(),
            sample_rate: DEFAULT_SAMPLE_RATE,
            stt_endpointing_ms: 400,
            stt_interim_results: true,
            client_identifier: String::new(),
            user_agent: String::new(),
        }
    }
}

impl VoiceConfig {
    /// Streaming STT WebSocket URL. Rejects plaintext `http://` / `ws://`.
    pub fn stt_ws_url(&self) -> Result<String, VoiceError> {
        ws_url(&self.api_base, &self.stt_ws_path)
    }

    /// `api_base`: `GROK_VOICE_BASE_URL` env, else non-empty `[voice].api_base`,
    /// else `[endpoints].xai_api_base_url` from `root`, else
    /// `resolved_endpoints_base`, else `https://api.x.ai`.
    ///
    /// `resolved_endpoints_base` carries the caller's env / CLI overrides for
    /// `[endpoints]`; it ranks below the raw table so that key keeps its
    /// config-beats-env precedence. The voice-specific env key sits above the
    /// table on purpose: it is the per-run escape hatch for pointing STT at a
    /// gateway without editing config.
    pub fn from_config_table(root: &toml::Table, resolved_endpoints_base: Option<&str>) -> Self {
        Self::from_config_table_with_env(
            root,
            resolved_endpoints_base,
            std::env::var(VOICE_BASE_URL_ENV).ok().as_deref(),
        )
    }

    /// [`Self::from_config_table`] with the env tier passed in explicitly, for
    /// callers (and tests) that own the process environment.
    pub fn from_config_table_with_env(
        root: &toml::Table,
        resolved_endpoints_base: Option<&str>,
        env_base: Option<&str>,
    ) -> Self {
        let voice_table = root.get("voice").and_then(|v| v.as_table());
        let mut cfg: Self = voice_table
            .and_then(|t| toml::Value::Table(t.clone()).try_into().ok())
            .unwrap_or_default();

        // Read `[voice].api_base` from the raw table, not `cfg`: serde default
        // makes "unset" and an explicit `https://api.x.ai` indistinguishable.
        cfg.api_base = non_empty_str(env_base)
            .or_else(|| {
                non_empty_str(
                    voice_table
                        .and_then(|t| t.get("api_base"))
                        .and_then(|v| v.as_str()),
                )
            })
            .or_else(|| {
                non_empty_str(
                    root.get("endpoints")
                        .and_then(|e| e.get("xai_api_base_url"))
                        .and_then(|v| v.as_str()),
                )
            })
            .or_else(|| non_empty_str(resolved_endpoints_base))
            .map(|base| base.trim_end_matches('/').to_owned())
            .unwrap_or_else(|| Self::default().api_base);
        cfg
    }
}

fn non_empty_str(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

/// `strip_prefix` ignoring ASCII case: RFC 3986 schemes are case-insensitive,
/// so `HTTP://` must hit the plaintext rejection and `HTTPS://` must work.
fn strip_scheme<'a>(s: &'a str, scheme: &str) -> Option<&'a str> {
    s.get(..scheme.len())
        .filter(|p| p.eq_ignore_ascii_case(scheme))
        .map(|_| &s[scheme.len()..])
}

fn ws_url(api_base: &str, path: &str) -> Result<String, VoiceError> {
    let base = api_base.trim().trim_end_matches('/');
    let path = path.trim().trim_start_matches('/');
    if strip_scheme(base, "http://").is_some() || strip_scheme(base, "ws://").is_some() {
        return Err(VoiceError::Config(format!(
            "insecure voice api_base {api_base:?}: voice requires a TLS endpoint \
             (https:// / wss://). Refusing to send the bearer token over a \
             plaintext connection. Set {env} (or [voice].api_base) to an https:// \
             endpoint.",
            env = VOICE_BASE_URL_ENV,
        )));
    }
    let rest = strip_scheme(base, "https://")
        .or_else(|| strip_scheme(base, "wss://"))
        .unwrap_or(base);
    // Default path `/v1/stt`; bases often end in `/v1` or `/xai/v1`.
    let path = match (rest.ends_with("/v1"), path.strip_prefix("v1/")) {
        (true, Some(rest_path)) => rest_path,
        _ => path,
    };
    Ok(format!("wss://{rest}/{path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_stt_ws_uses_wss() {
        assert_eq!(
            VoiceConfig::default().stt_ws_url().unwrap(),
            "wss://api.x.ai/v1/stt"
        );
    }

    #[test]
    fn scheme_less_and_wss_bases() {
        for base in ["api.x.ai", "wss://api.x.ai", "HTTPS://api.x.ai"] {
            let cfg = VoiceConfig {
                api_base: base.into(),
                ..VoiceConfig::default()
            };
            assert_eq!(cfg.stt_ws_url().unwrap(), "wss://api.x.ai/v1/stt");
        }
    }

    #[test]
    fn v1_base_dedupes_default_path() {
        let cfg = VoiceConfig {
            api_base: "https://proxy.example.com/v1".into(),
            ..VoiceConfig::default()
        };
        assert_eq!(cfg.stt_ws_url().unwrap(), "wss://proxy.example.com/v1/stt");
    }

    #[test]
    fn xai_v1_base_preserves_prefix() {
        let cfg = VoiceConfig {
            api_base: "https://proxy.example.com/xai/v1".into(),
            ..VoiceConfig::default()
        };
        assert_eq!(
            cfg.stt_ws_url().unwrap(),
            "wss://proxy.example.com/xai/v1/stt"
        );
    }

    #[test]
    fn rejects_plaintext_bases() {
        for base in [
            "http://localhost:8080",
            "ws://localhost:8080",
            "HTTP://localhost:8080",
            "Ws://localhost:8080",
        ] {
            let cfg = VoiceConfig {
                api_base: base.into(),
                ..VoiceConfig::default()
            };
            let err = cfg.stt_ws_url().expect_err("plaintext base must be rejected");
            assert!(
                matches!(err, VoiceError::Config(_)),
                "expected a config error, got {err:?}"
            );
            // The remedy names the key, so a local gateway user is not stuck
            // guessing why `http://localhost:8080` was refused.
            assert!(err.to_string().contains(VOICE_BASE_URL_ENV), "got: {err}");
        }
    }

    #[test]
    fn inherits_endpoints_when_voice_api_base_unset() {
        let table: toml::Table = toml::from_str(
            r#"
[endpoints]
xai_api_base_url = "https://proxy.example.com/xai/v1"
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table_with_env(&table, None, None);
        assert_eq!(cfg.api_base, "https://proxy.example.com/xai/v1");
        assert_eq!(
            cfg.stt_ws_url().unwrap(),
            "wss://proxy.example.com/xai/v1/stt"
        );
    }

    #[test]
    fn empty_voice_api_base_still_inherits_endpoints() {
        let table: toml::Table = toml::from_str(
            r#"
[endpoints]
xai_api_base_url = "https://proxy.example.com/xai/v1"
[voice]
api_base = "  "
language = "fr"
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table_with_env(&table, None, None);
        assert_eq!(cfg.api_base, "https://proxy.example.com/xai/v1");
        assert_eq!(cfg.language, "fr");
    }

    #[test]
    fn whitespace_voice_api_base_without_endpoints_uses_default() {
        let table: toml::Table = toml::from_str(
            r#"
[voice]
api_base = "  "
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table_with_env(&table, None, None);
        assert_eq!(cfg.api_base, VoiceConfig::default().api_base);
        assert_eq!(cfg.stt_ws_url().unwrap(), "wss://api.x.ai/v1/stt");
    }

    #[test]
    fn resolved_endpoints_base_used_when_table_has_none() {
        let cfg = VoiceConfig::from_config_table_with_env(
            &toml::Table::new(),
            Some("https://proxy.example.com/v1/"),
            None,
        );
        assert_eq!(cfg.api_base, "https://proxy.example.com/v1");
        assert_eq!(cfg.stt_ws_url().unwrap(), "wss://proxy.example.com/v1/stt");

        // Whitespace-only resolved base falls through to the default.
        let cfg = VoiceConfig::from_config_table_with_env(&toml::Table::new(), Some("  "), None);
        assert_eq!(cfg.api_base, VoiceConfig::default().api_base);
    }

    /// config.toml beats the env/CLI fallback (shell endpoints precedence).
    #[test]
    fn table_endpoints_beat_resolved_endpoints_base() {
        let table: toml::Table = toml::from_str(
            r#"
[endpoints]
xai_api_base_url = "https://config.example.com"
"#,
        )
        .unwrap();
        let cfg =
            VoiceConfig::from_config_table_with_env(&table, Some("https://env.example.com"), None);
        assert_eq!(cfg.api_base, "https://config.example.com");
    }

    #[test]
    fn voice_api_base_overrides_endpoints() {
        let table: toml::Table = toml::from_str(
            r#"
[endpoints]
xai_api_base_url = "https://proxy.example.com/xai/v1"
[voice]
api_base = "https://api.x.ai"
language = "es"
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table_with_env(&table, None, None);
        assert_eq!(cfg.api_base, "https://api.x.ai");
        assert_eq!(cfg.language, "es");
        assert_eq!(cfg.stt_ws_url().unwrap(), "wss://api.x.ai/v1/stt");
    }

    #[test]
    fn ignores_unknown_and_identity_fields() {
        let table: toml::Table = toml::from_str(
            r#"
[voice]
enabled = false
client_identifier = "spoofed"
user_agent = "malicious/9.9"
language = "es"
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table_with_env(&table, None, None);
        assert_eq!(cfg.language, "es");
        assert!(cfg.client_identifier.is_empty());
        assert!(cfg.user_agent.is_empty());
    }

    /// `GROK_VOICE_BASE_URL` outranks every config tier: STT is its own
    /// surface, so a gateway that serves chat must not be assumed to serve
    /// `/stt`.
    #[test]
    fn env_base_url_outranks_config_tiers() {
        let table: toml::Table = toml::from_str(
            r#"
[endpoints]
xai_api_base_url = "https://config.example.com/xai/v1"
[voice]
api_base = "https://voice.example.com"
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table_with_env(
            &table,
            Some("https://resolved.example.com"),
            Some("  https://env.example.com/v1/  "),
        );
        assert_eq!(cfg.api_base, "https://env.example.com/v1");
        assert_eq!(
            cfg.stt_ws_url().unwrap(),
            "wss://env.example.com/v1/stt",
            "env override must move the STT socket"
        );
    }

    /// A blank env value falls through to config instead of pinning an empty
    /// base URL.
    #[test]
    fn blank_env_base_url_falls_through() {
        let table: toml::Table = toml::from_str(
            r#"
[voice]
api_base = "https://voice.example.com"
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table_with_env(&table, None, Some("   "));
        assert_eq!(cfg.api_base, "https://voice.example.com");
    }

    /// The env-reading entry point wires `GROK_VOICE_BASE_URL` through.
    #[test]
    fn from_config_table_reads_env_var() {
        // `std::env` is process-wide; nextest isolates tests in processes.
        let previous = std::env::var(VOICE_BASE_URL_ENV).ok();
        unsafe { std::env::set_var(VOICE_BASE_URL_ENV, "https://env-only.example.com") };
        let cfg = VoiceConfig::from_config_table(&toml::Table::new(), None);
        match previous {
            Some(value) => unsafe { std::env::set_var(VOICE_BASE_URL_ENV, value) },
            None => unsafe { std::env::remove_var(VOICE_BASE_URL_ENV) },
        }
        assert_eq!(cfg.api_base, "https://env-only.example.com");
        assert_eq!(
            cfg.stt_ws_url().unwrap(),
            "wss://env-only.example.com/v1/stt"
        );
    }
}
