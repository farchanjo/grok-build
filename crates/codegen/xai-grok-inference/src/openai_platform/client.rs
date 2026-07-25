//! Application, administration, and OpenRouter platform client facades.

use super::error::{PlatformError, PlatformResult};
use super::transport::{
    CredentialKind, CredentialResolver, ExtraHeaders, PlatformTransport, StaticCredentials,
    TransportPolicy,
};
use super::url_policy::NormalizedBaseUrl;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Configuration shared by platform clients.
#[derive(Debug, Clone)]
pub struct PlatformClientConfig {
    pub provider_id: String,
    pub display_name: String,
    pub base_url: String,
    /// Optional separate administration base URL. Defaults to `base_url`.
    pub admin_base_url: Option<String>,
    pub application_token: Option<String>,
    pub admin_token: Option<String>,
    pub extra_headers: ExtraHeaders,
    pub policy: TransportPolicy,
}

impl PlatformClientConfig {
    pub fn validate(&self) -> PlatformResult<()> {
        NormalizedBaseUrl::parse(&self.base_url)?;
        if let Some(admin) = &self.admin_base_url {
            NormalizedBaseUrl::parse(admin)?;
        }
        if self.provider_id.trim().is_empty() {
            return Err(PlatformError::InvalidRequest(
                "provider_id must not be empty".into(),
            ));
        }
        Ok(())
    }
}

/// OpenAI application API client (never injects admin credentials).
#[derive(Clone)]
pub struct OpenAiClient {
    pub(crate) transport: PlatformTransport,
}

/// OpenAI administration API client (never injects application credentials).
#[derive(Clone)]
pub struct OpenAiAdminClient {
    pub(crate) transport: PlatformTransport,
}

/// OpenRouter-native client (separate from OpenAI administration).
#[derive(Clone)]
pub struct OpenRouterClient {
    pub(crate) transport: PlatformTransport,
}

impl OpenAiClient {
    pub fn from_config(config: PlatformClientConfig, cancel: CancellationToken) -> PlatformResult<Self> {
        config.validate()?;
        let creds = Arc::new(StaticCredentials {
            application: config.application_token,
            admin: None,
        });
        let transport = PlatformTransport::new(
            &config.base_url,
            config.provider_id,
            config.display_name,
            creds,
            config.extra_headers,
            config.policy,
            cancel,
        )?;
        Ok(Self { transport })
    }

    pub fn from_transport(transport: PlatformTransport) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &PlatformTransport {
        &self.transport
    }

    /// Structural guard: application client never resolves admin credentials.
    pub fn resolve_application_token(&self) -> PlatformResult<Option<String>> {
        self.transport.credentials.resolve(CredentialKind::Application)
    }
}

impl OpenAiAdminClient {
    pub fn from_config(config: PlatformClientConfig, cancel: CancellationToken) -> PlatformResult<Self> {
        config.validate()?;
        let base = config
            .admin_base_url
            .as_deref()
            .unwrap_or(config.base_url.as_str());
        // Admin client is constructed with ONLY the admin token in the
        // application slot of StaticCredentials so accidental Application
        // credential kind still cannot reach the user API key.
        let admin_only = Arc::new(AdminOnlyCredentials {
            admin: config.admin_token,
        });
        let transport = PlatformTransport::new(
            base,
            config.provider_id,
            config.display_name,
            admin_only,
            config.extra_headers,
            config.policy,
            cancel,
        )?;
        Ok(Self { transport })
    }

    pub fn from_transport(transport: PlatformTransport) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &PlatformTransport {
        &self.transport
    }
}

impl OpenRouterClient {
    pub fn from_config(config: PlatformClientConfig, cancel: CancellationToken) -> PlatformResult<Self> {
        config.validate()?;
        let creds = Arc::new(StaticCredentials {
            application: config.application_token,
            admin: None,
        });
        let transport = PlatformTransport::new(
            &config.base_url,
            config.provider_id,
            config.display_name,
            creds,
            config.extra_headers,
            config.policy,
            cancel,
        )?;
        Ok(Self { transport })
    }

    pub fn from_transport(transport: PlatformTransport) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &PlatformTransport {
        &self.transport
    }
}

/// Credential resolver that only ever returns admin keys, for both kinds.
/// Application kind is treated as admin so generated admin ops that still
/// pass CredentialKind::Admin work, while no application user key is stored.
struct AdminOnlyCredentials {
    admin: Option<String>,
}

impl CredentialResolver for AdminOnlyCredentials {
    fn resolve(&self, kind: CredentialKind) -> PlatformResult<Option<String>> {
        match kind {
            CredentialKind::Admin | CredentialKind::Application => Ok(self.admin.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_client_rejects_empty_provider_id() {
        let cfg = PlatformClientConfig {
            provider_id: "  ".into(),
            display_name: "X".into(),
            base_url: "https://api.openai.com/v1".into(),
            admin_base_url: None,
            application_token: Some("k".into()),
            admin_token: None,
            extra_headers: Default::default(),
            policy: TransportPolicy::default(),
        };
        assert!(OpenAiClient::from_config(cfg, CancellationToken::new()).is_err());
    }

    #[test]
    fn admin_only_credentials_never_expose_user_key() {
        let c = AdminOnlyCredentials {
            admin: Some("admin-key".into()),
        };
        assert_eq!(
            c.resolve(CredentialKind::Admin).unwrap().as_deref(),
            Some("admin-key")
        );
    }
}
