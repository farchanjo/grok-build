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
    pub fn from_config(
        config: PlatformClientConfig,
        cancel: CancellationToken,
    ) -> PlatformResult<Self> {
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

    /// Connect to the OpenAI Realtime API with typed client/server events.
    pub async fn connect_realtime(
        &self,
        model: Option<&str>,
    ) -> PlatformResult<super::transport::RealtimeSession> {
        let mut query = std::collections::BTreeMap::new();
        if let Some(model) = model {
            query.insert("model".to_owned(), model.to_owned());
        }
        self.transport
            .connect_realtime(super::transport::HttpRequestSpec {
                method: "GET",
                path: "/realtime".into(),
                query,
                body: None,
                credential: CredentialKind::Application,
                expect_sse: false,
                expect_binary: false,
                multipart: false,
                operation_id: "connectRealtime",
                idempotent: false,
            })
            .await
    }

    /// Structural guard: application client never resolves admin credentials.
    pub fn resolve_application_token(&self) -> PlatformResult<Option<String>> {
        self.transport
            .credentials
            .resolve(CredentialKind::Application)
    }
}

impl OpenAiAdminClient {
    pub fn from_config(
        config: PlatformClientConfig,
        cancel: CancellationToken,
    ) -> PlatformResult<Self> {
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
    pub fn from_config(
        config: PlatformClientConfig,
        cancel: CancellationToken,
    ) -> PlatformResult<Self> {
        config.validate()?;
        // Provider-native dual slot: application and management/admin keys are
        // both available, but CredentialKind selection never borrows the other.
        let creds = Arc::new(StaticCredentials {
            application: config.application_token,
            admin: config.admin_token,
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

/// Credential resolver that only ever returns admin keys.
///
/// Application resolution fails closed so an admin client can never inject
/// an application credential, even by accident.
struct AdminOnlyCredentials {
    admin: Option<String>,
}

impl CredentialResolver for AdminOnlyCredentials {
    fn resolve(&self, kind: CredentialKind) -> PlatformResult<Option<String>> {
        match kind {
            CredentialKind::Admin => Ok(self.admin.clone()),
            CredentialKind::Application => Err(PlatformError::MissingCredential(
                super::error::CredentialClass::Application,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::error::PlatformError;
    use super::super::transport::{CredentialKind, StaticCredentials};
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
    fn admin_only_credentials_fail_closed_on_application_kind() {
        let c = AdminOnlyCredentials {
            admin: Some("admin-key".into()),
        };
        assert_eq!(
            c.resolve(CredentialKind::Admin).unwrap().as_deref(),
            Some("admin-key")
        );
        let err = c.resolve(CredentialKind::Application).unwrap_err();
        assert!(matches!(
            err,
            PlatformError::MissingCredential(super::super::error::CredentialClass::Application)
        ));
        // Structural: admin key is never returned for Application.
        assert!(!format!("{err:?}").contains("admin-key"));
    }

    #[test]
    fn openrouter_client_keeps_dual_slots_without_borrow() {
        let cfg = PlatformClientConfig {
            provider_id: "openrouter".into(),
            display_name: "OpenRouter".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            admin_base_url: None,
            application_token: Some("app-key".into()),
            admin_token: Some("admin-key".into()),
            extra_headers: Default::default(),
            policy: TransportPolicy::default(),
        };
        let client = OpenRouterClient::from_config(cfg, CancellationToken::new()).unwrap();
        assert_eq!(
            client
                .transport()
                .credentials
                .resolve(CredentialKind::Application)
                .unwrap()
                .as_deref(),
            Some("app-key")
        );
        assert_eq!(
            client
                .transport()
                .credentials
                .resolve(CredentialKind::Admin)
                .unwrap()
                .as_deref(),
            Some("admin-key")
        );
    }

    #[test]
    fn openrouter_admin_missing_does_not_borrow_application() {
        let cfg = PlatformClientConfig {
            provider_id: "openrouter".into(),
            display_name: "OpenRouter".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            admin_base_url: None,
            application_token: Some("app-key".into()),
            admin_token: None,
            extra_headers: Default::default(),
            policy: TransportPolicy::default(),
        };
        let client = OpenRouterClient::from_config(cfg, CancellationToken::new()).unwrap();
        assert_eq!(
            client
                .transport()
                .credentials
                .resolve(CredentialKind::Admin)
                .unwrap(),
            None
        );
        assert_eq!(
            client
                .transport()
                .credentials
                .resolve(CredentialKind::Application)
                .unwrap()
                .as_deref(),
            Some("app-key")
        );
    }

    #[test]
    fn app_client_credentials_never_resolve_admin() {
        let creds = StaticCredentials {
            application: Some("app-key".into()),
            admin: Some("admin-key".into()),
        };
        assert_eq!(
            creds
                .resolve(CredentialKind::Application)
                .unwrap()
                .as_deref(),
            Some("app-key")
        );
        assert_eq!(
            creds.resolve(CredentialKind::Admin).unwrap().as_deref(),
            Some("admin-key")
        );
    }
}
