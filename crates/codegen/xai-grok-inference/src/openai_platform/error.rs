//! Normalized platform client errors (no credential or body leakage).

use std::fmt;

/// Result alias for platform operations.
pub type PlatformResult<T> = Result<T, PlatformError>;

/// Fail-closed platform error. Display and debug forms never include
/// authorization headers, raw request bodies, or secret-bearing URLs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformError {
    InvalidRequest(String),
    InvalidUrl(String),
    MissingCredential(CredentialClass),
    /// Cross-origin redirect refused with credentials stripped.
    RedirectPolicy(String),
    Http {
        status: u16,
        /// Safe provider-facing category (never a raw body).
        category: ErrorCategory,
        /// Bounded, redacted message suitable for UI/CLI.
        message: String,
        request_id: Option<String>,
        operation_id: Option<String>,
        provider_id: Option<String>,
    },
    Decode(String),
    Timeout {
        operation_id: Option<String>,
    },
    Cancelled,
    RateLimited {
        retry_after_ms: Option<u64>,
        request_id: Option<String>,
        operation_id: Option<String>,
    },
    OversizedResponse {
        limit_bytes: usize,
    },
    PaginationLimit,
    Transport(String),
    UnsupportedTransport(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialClass {
    Application,
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Authentication,
    Authorization,
    NotFound,
    Conflict,
    Validation,
    RateLimit,
    Server,
    PaymentRequired,
    Unknown,
}

impl ErrorCategory {
    pub fn from_status(status: u16) -> Self {
        match status {
            401 => Self::Authentication,
            403 => Self::Authorization,
            404 => Self::NotFound,
            409 => Self::Conflict,
            402 => Self::PaymentRequired,
            422 | 400 => Self::Validation,
            429 => Self::RateLimit,
            s if (500..600).contains(&s) => Self::Server,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Authentication => "authentication",
            Self::Authorization => "authorization",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Validation => "validation",
            Self::RateLimit => "rate_limit",
            Self::Server => "server",
            Self::PaymentRequired => "payment_required",
            Self::Unknown => "unknown",
        }
    }
}

impl PlatformError {
    pub fn is_reauthable(&self) -> bool {
        matches!(
            self,
            Self::Http {
                category: ErrorCategory::Authentication,
                ..
            } | Self::MissingCredential(_)
        )
    }

    /// Safe single-line message for UI and CLI (no secrets).
    pub fn safe_message(&self) -> String {
        match self {
            Self::InvalidRequest(m) => format!("invalid request: {m}"),
            Self::InvalidUrl(m) => format!("invalid url: {m}"),
            Self::MissingCredential(CredentialClass::Application) => {
                "missing application credential for provider".into()
            }
            Self::MissingCredential(CredentialClass::Admin) => {
                "missing administration credential for provider".into()
            }
            Self::RedirectPolicy(m) => format!("redirect refused: {m}"),
            Self::Http {
                status,
                category,
                message,
                request_id,
                provider_id,
                ..
            } => {
                let mut out = format!(
                    "HTTP {status} ({}){}",
                    category.as_str(),
                    provider_id
                        .as_ref()
                        .map(|p| format!(" provider={p}"))
                        .unwrap_or_default()
                );
                if let Some(id) = request_id {
                    out.push_str(&format!(" request_id={id}"));
                }
                if !message.is_empty() {
                    out.push_str(": ");
                    out.push_str(message);
                }
                out
            }
            Self::Decode(m) => format!("decode error: {m}"),
            Self::Timeout { operation_id } => match operation_id {
                Some(op) => format!("timeout on {op}"),
                None => "timeout".into(),
            },
            Self::Cancelled => "cancelled".into(),
            Self::RateLimited {
                retry_after_ms,
                request_id,
                ..
            } => {
                let mut out = "rate limited".to_string();
                if let Some(ms) = retry_after_ms {
                    out.push_str(&format!(" retry_after_ms={ms}"));
                }
                if let Some(id) = request_id {
                    out.push_str(&format!(" request_id={id}"));
                }
                out
            }
            Self::OversizedResponse { limit_bytes } => {
                format!("response exceeded {limit_bytes} byte limit")
            }
            Self::PaginationLimit => "pagination loop protection limit reached".into(),
            Self::Transport(m) => format!("transport error: {m}"),
            Self::UnsupportedTransport(m) => format!("unsupported transport: {m}"),
        }
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.safe_message())
    }
}

impl std::error::Error for PlatformError {}

/// Redact obvious secret patterns from an error preview string.
pub fn redact_preview(input: &str, max_chars: usize) -> String {
    let mut out = input.to_string();
    for needle in [
        "Authorization:",
        "authorization:",
        "Bearer ",
        "api-key:",
        "api_key=",
        "sk-",
        "x-api-key",
    ] {
        if let Some(idx) = out.to_ascii_lowercase().find(&needle.to_ascii_lowercase()) {
            let end = (idx + needle.len() + 8).min(out.len());
            out.replace_range(idx..end, "[redacted]");
        }
    }
    if out.chars().count() > max_chars {
        let trimmed: String = out.chars().take(max_chars).collect();
        format!("{trimmed}…")
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_never_embeds_bearer_token() {
        let err = PlatformError::Http {
            status: 401,
            category: ErrorCategory::Authentication,
            message: redact_preview("Bearer sk-secret-value-here rejected", 80),
            request_id: Some("req_1".into()),
            operation_id: Some("listModels".into()),
            provider_id: Some("openai".into()),
        };
        let s = err.to_string();
        assert!(!s.contains("sk-secret"));
        assert!(s.contains("provider=openai"));
        assert!(s.contains("request_id=req_1"));
    }
}
