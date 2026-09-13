//! Asset-store configuration value types (`[assets]`, `[assets_providers.<id>]`).
//!
//! Pure, credential-free DTOs for the multi-backend object-storage subsystem.
//! The runtime trait and the backend adapters live in `xai-file-utils`; this
//! module owns only the wire schema, its defaults, and the bounds that the
//! resolver and the management surface share.
//!
//! Invariants enforced here:
//!
//! - `[assets]` is tolerant: an unknown key never fails the parse, so a newer
//!   config cannot break an older binary.
//! - `[assets_providers.<id>]` is strict (`deny_unknown_fields`): a typo in a
//!   named profile is a hard error rather than a silently ignored setting.
//! - A named profile carries only *overrides*; every unset field inherits from
//!   `[assets]`. `kind` is the one required field.
//! - Presign TTL bounds are `1s ..= 604_800s` (the SigV4 ceiling). Out of range
//!   is an error, never a silent clamp.
//! - No secret ever appears here: `env_key` holds an environment-variable
//!   *name*, `credentials_file` a path.

use std::time::Duration;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Bounds (shared by parse-time validation, the resolver, and management)
// ---------------------------------------------------------------------------

/// Shortest accepted presign TTL.
pub const MIN_ASSET_TTL_SECS: u64 = 1;
/// Longest accepted presign TTL — the SigV4 presign ceiling.
pub const MAX_ASSET_TTL_SECS: u64 = 604_800;
/// Default presign TTL (one hour).
pub const DEFAULT_ASSET_TTL_SECS: u64 = 3_600;

/// Default key namespace. Every derived object key starts with this.
pub const DEFAULT_ASSET_KEY_PREFIX: &str = "uploads/";

/// Default per-request timeout for backend calls.
pub const DEFAULT_ASSET_REQUEST_TIMEOUT_SECS: u64 = 30;

/// Longest accepted `[assets_providers.<id>]` id.
pub const MAX_ASSET_PROFILE_ID_LEN: usize = 64;

/// Smallest accepted `ListQuery::limit`.
pub const MIN_ASSET_LIST_LIMIT: u32 = 1;
/// Largest accepted `ListQuery::limit`.
pub const MAX_ASSET_LIST_LIMIT: u32 = 1_000;
/// Default `ListQuery::limit`.
pub const DEFAULT_ASSET_LIST_LIMIT: u32 = 100;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Default visibility for a newly stored object.
///
/// Private is the default and public is always explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetVisibility {
    #[default]
    Private,
    Public,
}

impl AssetVisibility {
    /// Stable snake_case wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Public => "public",
        }
    }

    /// Parse the snake_case wire name. Case-insensitive on the ASCII alphabet.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "private" => Some(Self::Private),
            "public" => Some(Self::Public),
            _ => None,
        }
    }

    pub const fn is_public(self) -> bool {
        matches!(self, Self::Public)
    }
}

/// Backend kind for `[assets].provider` and `[assets_providers.<id>].kind`.
///
/// Static four-way enum: the set of backends is a compile-time decision, so a
/// typo fails the parse instead of degrading at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetProviderKind {
    S3,
    Gcs,
    Local,
    Proxy,
}

impl AssetProviderKind {
    /// Stable snake_case wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::S3 => "s3",
            Self::Gcs => "gcs",
            Self::Local => "local",
            Self::Proxy => "proxy",
        }
    }

    /// Parse the snake_case wire name.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "s3" => Some(Self::S3),
            "gcs" => Some(Self::Gcs),
            "local" => Some(Self::Local),
            "proxy" => Some(Self::Proxy),
            _ => None,
        }
    }

    /// True for backends that need a bucket and network credentials.
    pub const fn is_object_store(self) -> bool {
        matches!(self, Self::S3 | Self::Gcs)
    }
}

impl std::fmt::Display for AssetProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// `[assets]`
// ---------------------------------------------------------------------------

/// The `[assets]` table.
///
/// Tolerant by design: no `deny_unknown_fields`, so additive keys roll out
/// safely. Every field is optional except the ones with a documented default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsConfig {
    /// Explicit backend selection. Wins over every other selection signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<AssetProviderKind>,
    /// Named `[assets_providers.<id>]` profile to activate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    /// Bucket name, optionally scheme-qualified (`s3://bucket`, `gs://bucket`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    /// Region for the object store.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// Custom endpoint (S3-compatible services, emulators).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_url: Option<String>,
    /// Path to a credentials file. Never the credential itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials_file: Option<String>,
    /// Name of the environment variable holding the credential.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_key: Option<String>,
    /// Public base URL used for `public_url` and recorded visibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_base_url: Option<String>,
    /// Visibility applied when a caller does not specify one.
    pub default_visibility: AssetVisibility,
    /// Presign TTL applied when a caller does not specify one.
    pub default_ttl_secs: u64,
    /// Local backend root. Defaults to `$GROK_HOME/assets`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_root: Option<String>,
    /// Base URL of the upload proxy backend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_base_url: Option<String>,
    /// Reject objects larger than this. `None` = no limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_object_bytes: Option<u64>,
    /// Per-request timeout for backend calls.
    pub request_timeout_secs: u64,
    /// Namespace prepended to every derived object key.
    pub key_prefix: String,
}

impl Default for AssetsConfig {
    fn default() -> Self {
        Self {
            provider: None,
            active_profile: None,
            bucket: None,
            region: None,
            endpoint_url: None,
            credentials_file: None,
            env_key: None,
            public_base_url: None,
            default_visibility: AssetVisibility::default(),
            default_ttl_secs: DEFAULT_ASSET_TTL_SECS,
            local_root: None,
            proxy_base_url: None,
            max_object_bytes: None,
            request_timeout_secs: DEFAULT_ASSET_REQUEST_TIMEOUT_SECS,
            key_prefix: DEFAULT_ASSET_KEY_PREFIX.to_owned(),
        }
    }
}

// ---------------------------------------------------------------------------
// `[assets_providers.<id>]`
// ---------------------------------------------------------------------------

/// One named backend profile.
///
/// Every field except `kind` is an override: `None` inherits from `[assets]`.
/// Strict (`deny_unknown_fields`) so a misspelled override is loud.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetProviderConfig {
    /// Required backend kind for this profile.
    pub kind: AssetProviderKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_visibility: Option<AssetVisibility>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_ttl_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_object_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_timeout_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_prefix: Option<String>,
}

/// Named asset profiles, keyed by profile id. Insertion order is preserved so
/// resolution and comment-preserving writes stay deterministic.
pub type AssetProviderCatalog = IndexMap<String, AssetProviderConfig>;

// ---------------------------------------------------------------------------
// Aggregate
// ---------------------------------------------------------------------------

/// The whole asset configuration surface: `[assets]` plus
/// `[assets_providers.<id>]`.
///
/// Deserializes directly from a config fragment, so a host config can embed it
/// with `#[serde(flatten)]` or parse the two tables in isolation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsSettings {
    pub assets: AssetsConfig,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub assets_providers: AssetProviderCatalog,
}

impl AssetsSettings {
    /// Look up a named profile.
    pub fn profile(&self, id: &str) -> Option<&AssetProviderConfig> {
        self.assets_providers.get(id)
    }

    /// Effective presign TTL for the base `[assets]` table.
    pub fn effective_ttl(&self) -> Result<Duration, String> {
        validate_asset_ttl_secs(self.assets.default_ttl_secs)
    }

    /// Effective key prefix for the base `[assets]` table.
    pub fn effective_key_prefix(&self) -> &str {
        &self.assets.key_prefix
    }

    /// Validate every cross-field invariant the resolver depends on.
    ///
    /// Returns the first secret-free diagnostic, or `Ok(())`. Nested profile
    /// errors are prefixed with the profile id so the message is actionable.
    pub fn validate(&self) -> Result<(), String> {
        validate_asset_ttl_secs(self.assets.default_ttl_secs)?;
        validate_asset_key_prefix(&self.assets.key_prefix)?;
        if let Some(active) = self.assets.active_profile.as_deref() {
            validate_asset_profile_id(active)?;
            if !self.assets_providers.contains_key(active) {
                return Err(format!(
                    "`assets.active_profile` names `{active}`, which has no \
                     `[assets_providers.{active}]` table"
                ));
            }
        }
        for (id, profile) in &self.assets_providers {
            validate_asset_profile_id(id).map_err(|e| format!("[assets_providers.{id}]: {e}"))?;
            if let Some(ttl) = profile.default_ttl_secs {
                validate_asset_ttl_secs(ttl)
                    .map_err(|e| format!("[assets_providers.{id}].default_ttl_secs: {e}"))?;
            }
            if let Some(prefix) = profile.key_prefix.as_deref() {
                validate_asset_key_prefix(prefix)
                    .map_err(|e| format!("[assets_providers.{id}].key_prefix: {e}"))?;
            }
            profile
                .validate_shape()
                .map_err(|e| format!("[assets_providers.{id}]: {e}"))?;
        }
        Ok(())
    }
}

impl AssetProviderConfig {
    /// Validate the profile's *own* fields for syntax.
    ///
    /// Required-field checks (a bucket for a remote backend, a base URL for the
    /// proxy) depend on what `[assets]` supplies, so the resolver runs them on
    /// the merged view rather than here.
    pub fn validate_shape(&self) -> Result<(), String> {
        match self.kind {
            AssetProviderKind::Local => {
                if self.local_root.as_deref().is_some_and(str::is_empty) {
                    return Err("`local_root` must not be empty when set".to_owned());
                }
            }
            AssetProviderKind::S3 | AssetProviderKind::Gcs | AssetProviderKind::Proxy => {
                if self.bucket.as_deref().is_some_and(str::is_empty) {
                    return Err("`bucket` must not be empty when set".to_owned());
                }
            }
        }
        if let Some(url) = self.endpoint_url.as_deref()
            && !(url.starts_with("http://") || url.starts_with("https://"))
        {
            return Err("`endpoint_url` must start with http:// or https://".to_owned());
        }
        if let Some(url) = self.proxy_base_url.as_deref()
            && !url.is_empty()
            && !(url.starts_with("http://") || url.starts_with("https://"))
        {
            return Err("`proxy_base_url` must start with http:// or https://".to_owned());
        }
        if let Some(url) = self.public_base_url.as_deref()
            && !url.is_empty()
            && !(url.starts_with("http://") || url.starts_with("https://"))
        {
            return Err("`public_base_url` must start with http:// or https://".to_owned());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Shared validators
// ---------------------------------------------------------------------------

/// Validate a presign TTL against the SigV4 bounds.
///
/// Out of range is an error, never a silent clamp.
pub fn validate_asset_ttl_secs(secs: u64) -> Result<Duration, String> {
    if secs < MIN_ASSET_TTL_SECS {
        return Err(format!(
            "`default_ttl_secs` must be at least {MIN_ASSET_TTL_SECS}s, got {secs}s"
        ));
    }
    if secs > MAX_ASSET_TTL_SECS {
        return Err(format!(
            "`default_ttl_secs` must be at most {MAX_ASSET_TTL_SECS}s (SigV4 ceiling), got {secs}s"
        ));
    }
    Ok(Duration::from_secs(secs))
}

/// True when `secs` is inside the accepted presign TTL window.
pub const fn asset_ttl_secs_in_range(secs: u64) -> bool {
    secs >= MIN_ASSET_TTL_SECS && secs <= MAX_ASSET_TTL_SECS
}

/// Validate a profile id: `[A-Za-z0-9_-]{1,64}`.
pub fn validate_asset_profile_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("profile id must not be empty".to_owned());
    }
    if id.len() > MAX_ASSET_PROFILE_ID_LEN {
        return Err(format!(
            "profile id must be at most {MAX_ASSET_PROFILE_ID_LEN} bytes, got {}",
            id.len()
        ));
    }
    if let Some(bad) = id
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '_' || *c == '-'))
    {
        return Err(format!(
            "profile id may only contain [A-Za-z0-9_-], found `{bad}`"
        ));
    }
    Ok(())
}

/// Validate a key prefix: either empty, or a `/`-terminated run of
/// `[A-Za-z0-9._~-]` segments.
pub fn validate_asset_key_prefix(prefix: &str) -> Result<(), String> {
    if prefix.is_empty() {
        return Ok(());
    }
    if !prefix.ends_with('/') {
        return Err(format!("`key_prefix` must end with `/`, got `{prefix}`"));
    }
    if prefix.starts_with('/') {
        return Err(format!(
            "`key_prefix` must not start with `/`, got `{prefix}`"
        ));
    }
    if prefix.contains("//") {
        return Err(format!(
            "`key_prefix` must not contain `//`, got `{prefix}`"
        ));
    }
    for segment in prefix.split('/').filter(|s| !s.is_empty()) {
        if segment == "." || segment == ".." {
            return Err(format!("`key_prefix` must not contain `.`/`..` segments"));
        }
        if segment == "_meta" {
            return Err(
                "`key_prefix` must not start with the reserved `_meta/` namespace".to_owned(),
            );
        }
        if let Some(bad) = segment
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '~' | '-')))
        {
            return Err(format!(
                "`key_prefix` may only contain [A-Za-z0-9._~-] and `/`, found `{bad}`"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_private_local_and_bounded() {
        let config = AssetsConfig::default();
        assert_eq!(config.provider, None);
        assert_eq!(config.active_profile, None);
        assert_eq!(config.bucket, None);
        assert_eq!(config.default_visibility, AssetVisibility::Private);
        assert_eq!(config.default_ttl_secs, DEFAULT_ASSET_TTL_SECS);
        assert_eq!(
            config.request_timeout_secs,
            DEFAULT_ASSET_REQUEST_TIMEOUT_SECS
        );
        assert_eq!(config.key_prefix, DEFAULT_ASSET_KEY_PREFIX);
        assert_eq!(config.max_object_bytes, None);
        assert!(config.local_root.is_none());

        let settings = AssetsSettings::default();
        assert!(settings.assets_providers.is_empty());
        settings.validate().expect("empty settings must validate");
    }

    #[test]
    fn visibility_defaults_to_private_and_round_trips() {
        assert_eq!(AssetVisibility::default(), AssetVisibility::Private);
        assert!(!AssetVisibility::default().is_public());
        assert_eq!(
            AssetVisibility::parse("PUBLIC"),
            Some(AssetVisibility::Public)
        );
        assert_eq!(
            AssetVisibility::parse(" private "),
            Some(AssetVisibility::Private)
        );
        assert_eq!(AssetVisibility::parse("shared"), None);
        for value in [AssetVisibility::Private, AssetVisibility::Public] {
            let json = serde_json::to_string(&value).unwrap();
            assert_eq!(
                serde_json::from_str::<AssetVisibility>(&json).unwrap(),
                value
            );
            assert_eq!(json, format!("\"{}\"", value.as_str()));
        }
        assert!(serde_json::from_str::<AssetVisibility>(r#""Public""#).is_err());
    }

    #[test]
    fn provider_kind_parses_only_known_names() {
        for kind in [
            AssetProviderKind::S3,
            AssetProviderKind::Gcs,
            AssetProviderKind::Local,
            AssetProviderKind::Proxy,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{}\"", kind.as_str()));
            assert_eq!(
                serde_json::from_str::<AssetProviderKind>(&json).unwrap(),
                kind
            );
        }
        assert_eq!(AssetProviderKind::parse("S3"), Some(AssetProviderKind::S3));
        assert_eq!(AssetProviderKind::parse("azure"), None);
        assert!(serde_json::from_str::<AssetProviderConfig>(r#"{"kind":"azure"}"#).is_err());
        assert!(AssetProviderKind::S3.is_object_store());
        assert!(!AssetProviderKind::Proxy.is_object_store());
    }

    #[test]
    fn assets_table_tolerates_unknown_fields() {
        let config: AssetsConfig =
            serde_json::from_str(r#"{"bucket":"b","future_knob":42,"nested":{"x":1}}"#).unwrap();
        assert_eq!(config.bucket.as_deref(), Some("b"));
        assert_eq!(config.default_ttl_secs, DEFAULT_ASSET_TTL_SECS);
    }

    #[test]
    fn profile_rejects_unknown_fields() {
        let ok: AssetProviderConfig =
            serde_json::from_str(r#"{"kind":"s3","bucket":"b"}"#).unwrap();
        assert_eq!(ok.kind, AssetProviderKind::S3);
        assert_eq!(ok.bucket.as_deref(), Some("b"));

        let err = serde_json::from_str::<AssetProviderConfig>(r#"{"kind":"s3","buckets":"b"}"#);
        assert!(err.is_err(), "misspelled profile key must fail the parse");
    }

    #[test]
    fn profile_requires_kind() {
        assert!(serde_json::from_str::<AssetProviderConfig>(r#"{"bucket":"b"}"#).is_err());
    }

    #[test]
    fn active_profile_round_trips_through_settings() {
        let toml_src = r#"
[assets]
active_profile = "team-media"
bucket = "shared"

[assets_providers.team-media]
kind = "s3"
bucket = "team-media"
region = "us-east-1"
"#;
        let settings: AssetsSettings = toml::from_str(toml_src).unwrap();
        assert_eq!(
            settings.assets.active_profile.as_deref(),
            Some("team-media")
        );
        assert_eq!(
            settings.profile("team-media").unwrap().kind,
            AssetProviderKind::S3
        );
        settings.validate().unwrap();

        let encoded = toml::to_string(&settings).unwrap();
        let round: AssetsSettings = toml::from_str(&encoded).unwrap();
        assert_eq!(round, settings);
        assert_eq!(round.assets.active_profile.as_deref(), Some("team-media"));
    }

    #[test]
    fn validate_rejects_dangling_active_profile() {
        let settings: AssetsSettings =
            serde_json::from_str(r#"{"assets":{"active_profile":"typo"}}"#).unwrap();
        let err = settings.validate().unwrap_err();
        assert!(err.contains("typo"), "{err}");
        assert!(err.contains("assets_providers.typo"), "{err}");
    }

    #[test]
    fn validate_rejects_bad_profile_id() {
        let settings: AssetsSettings =
            serde_json::from_str(r#"{"assets_providers":{"bad id":{"kind":"local"}}}"#).unwrap();
        assert!(settings.validate().unwrap_err().contains("bad id"));

        assert!(validate_asset_profile_id("").is_err());
        assert!(validate_asset_profile_id(&"a".repeat(65)).is_err());
        validate_asset_profile_id("a-b_C9").unwrap();
        validate_asset_profile_id(&"a".repeat(64)).unwrap();
    }

    #[test]
    fn ttl_bounds_accept_in_range_and_reject_out_of_range() {
        assert_eq!(validate_asset_ttl_secs(1).unwrap(), Duration::from_secs(1));
        assert_eq!(
            validate_asset_ttl_secs(3_600).unwrap(),
            Duration::from_secs(3_600)
        );
        assert_eq!(
            validate_asset_ttl_secs(MAX_ASSET_TTL_SECS).unwrap(),
            Duration::from_secs(MAX_ASSET_TTL_SECS)
        );

        // Out of range is an error, never a silent clamp.
        assert!(validate_asset_ttl_secs(0).is_err());
        assert!(validate_asset_ttl_secs(MAX_ASSET_TTL_SECS + 1).is_err());
        assert!(validate_asset_ttl_secs(u64::MAX).is_err());

        assert!(asset_ttl_secs_in_range(1));
        assert!(asset_ttl_secs_in_range(MAX_ASSET_TTL_SECS));
        assert!(!asset_ttl_secs_in_range(0));
        assert!(!asset_ttl_secs_in_range(MAX_ASSET_TTL_SECS + 1));
    }

    #[test]
    fn validate_rejects_out_of_range_ttl_on_base_and_profile() {
        let mut settings = AssetsSettings::default();
        settings.assets.default_ttl_secs = 0;
        assert!(settings.validate().unwrap_err().contains("at least 1s"));

        let mut settings = AssetsSettings::default();
        settings.assets.default_ttl_secs = MAX_ASSET_TTL_SECS + 1;
        assert!(settings.validate().unwrap_err().contains("SigV4 ceiling"));

        let settings: AssetsSettings = serde_json::from_str(
            r#"{"assets_providers":{"p":{"kind":"local","default_ttl_secs":604801}}}"#,
        )
        .unwrap();
        let err = settings.validate().unwrap_err();
        assert!(
            err.contains("[assets_providers.p].default_ttl_secs"),
            "{err}"
        );
    }

    #[test]
    fn key_prefix_validation_accepts_trailing_slash_and_rejects_traps() {
        validate_asset_key_prefix("").unwrap();
        validate_asset_key_prefix("uploads/").unwrap();
        validate_asset_key_prefix("a/b/").unwrap();

        assert!(
            validate_asset_key_prefix("uploads").is_err(),
            "missing trailing /"
        );
        assert!(validate_asset_key_prefix("/uploads/").is_err(), "leading /");
        assert!(validate_asset_key_prefix("a//b/").is_err(), "empty segment");
        assert!(
            validate_asset_key_prefix("a/../b/").is_err(),
            "dot-dot segment"
        );
        assert!(
            validate_asset_key_prefix("_meta/").is_err(),
            "reserved namespace"
        );
        assert!(validate_asset_key_prefix("a b/").is_err(), "space");

        let settings: AssetsSettings =
            serde_json::from_str(r#"{"assets":{"key_prefix":"uploads"}}"#).unwrap();
        assert!(settings.validate().unwrap_err().contains("key_prefix"));
    }

    #[test]
    fn profile_shape_checks_url_syntax_and_empty_overrides() {
        // A `proxy` profile may inherit `proxy_base_url` from `[assets]`, so
        // the missing-URL check belongs to the resolver, not here.
        let proxy: AssetProviderConfig = serde_json::from_str(r#"{"kind":"proxy"}"#).unwrap();
        proxy.validate_shape().unwrap();

        let s3: AssetProviderConfig =
            serde_json::from_str(r#"{"kind":"s3","endpoint_url":"s3.example"}"#).unwrap();
        assert!(s3.validate_shape().is_err());

        let empty_bucket: AssetProviderConfig =
            serde_json::from_str(r#"{"kind":"s3","bucket":""}"#).unwrap();
        assert!(empty_bucket.validate_shape().is_err());

        let bad_public: AssetProviderConfig =
            serde_json::from_str(r#"{"kind":"local","public_base_url":"cdn.test"}"#).unwrap();
        assert!(bad_public.validate_shape().is_err());
    }

    #[test]
    fn profile_inherits_from_assets_when_fields_are_unset() {
        let settings: AssetsSettings = toml::from_str(
            r#"
[assets]
region = "us-east-1"
default_ttl_secs = 60

[assets_providers.only-overrides]
kind = "gcs"
bucket = "team"
"#,
        )
        .unwrap();
        let profile = settings.profile("only-overrides").unwrap();
        assert_eq!(profile.region, None, "unset profile field inherits");
        assert_eq!(profile.kind, AssetProviderKind::Gcs);
        assert_eq!(settings.assets.region.as_deref(), Some("us-east-1"));
    }
}
