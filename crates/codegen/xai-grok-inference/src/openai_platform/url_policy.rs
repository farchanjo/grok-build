//! Same-origin URL joining and path-segment encoding for platform clients.

use super::error::{PlatformError, PlatformResult};
use std::str::FromStr;

/// Normalized base URL used for all platform requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedBaseUrl {
    /// Scheme + authority only (no path), e.g. `https://api.openai.com`.
    pub origin: String,
    /// Path prefix without trailing slash, e.g. `/v1`. Empty means root.
    pub path_prefix: String,
}

impl NormalizedBaseUrl {
    /// Parse and normalize a configured base URL.
    ///
    /// Rejects credentials embedded in the URL, fragments, and empty hosts.
    pub fn parse(raw: &str) -> PlatformResult<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(PlatformError::InvalidUrl("base URL is empty".into()));
        }
        let url = reqwest::Url::from_str(trimmed)
            .map_err(|e| PlatformError::InvalidUrl(format!("parse failed: {e}")))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(PlatformError::InvalidUrl(format!(
                "unsupported scheme `{}`",
                url.scheme()
            )));
        }
        if url.username() != "" || url.password().is_some() {
            return Err(PlatformError::InvalidUrl(
                "base URL must not embed credentials".into(),
            ));
        }
        if url.fragment().is_some() {
            return Err(PlatformError::InvalidUrl(
                "base URL must not include a fragment".into(),
            ));
        }
        let host = url
            .host_str()
            .ok_or_else(|| PlatformError::InvalidUrl("base URL missing host".into()))?;
        if host.is_empty() {
            return Err(PlatformError::InvalidUrl("base URL host is empty".into()));
        }
        let origin = match url.port() {
            Some(port) => format!("{}://{}:{port}", url.scheme(), host),
            None => format!("{}://{}", url.scheme(), host),
        };
        let mut path_prefix = url.path().trim_end_matches('/').to_string();
        if path_prefix == "/" {
            path_prefix.clear();
        }
        Ok(Self {
            origin,
            path_prefix,
        })
    }

    /// Join a relative API path (must start with `/`) onto this base.
    pub fn join_path(&self, relative: &str) -> PlatformResult<reqwest::Url> {
        if !relative.starts_with('/') {
            return Err(PlatformError::InvalidUrl(
                "endpoint path must be absolute (start with /)".into(),
            ));
        }
        if relative.contains("://") || relative.contains("..") {
            return Err(PlatformError::InvalidUrl(
                "endpoint path must not contain scheme or parent segments".into(),
            ));
        }
        let full = format!("{}{}{}", self.origin, self.path_prefix, relative);
        reqwest::Url::from_str(&full)
            .map_err(|e| PlatformError::InvalidUrl(format!("join failed: {e}")))
    }

    /// True when `candidate` shares scheme + host + port with this base.
    pub fn same_origin(&self, candidate: &reqwest::Url) -> bool {
        let Ok(base) = reqwest::Url::from_str(&self.origin) else {
            return false;
        };
        base.scheme() == candidate.scheme()
            && base.host_str() == candidate.host_str()
            && base.port_or_known_default() == candidate.port_or_known_default()
    }
}

/// Encode a single path segment (never full paths).
pub fn encode_path_segment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for b in raw.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_base_with_v1_prefix() {
        let base = NormalizedBaseUrl::parse("https://api.openai.com/v1/").unwrap();
        assert_eq!(base.origin, "https://api.openai.com");
        assert_eq!(base.path_prefix, "/v1");
        let url = base.join_path("/models").unwrap();
        assert_eq!(url.as_str(), "https://api.openai.com/v1/models");
    }

    #[test]
    fn rejects_embedded_credentials() {
        let err = NormalizedBaseUrl::parse("https://user:pass@api.example/v1").unwrap_err();
        assert!(matches!(err, PlatformError::InvalidUrl(_)));
    }

    #[test]
    fn rejects_parent_segments() {
        let base = NormalizedBaseUrl::parse("http://127.0.0.1:8000/v1").unwrap();
        assert!(base.join_path("/../secret").is_err());
    }

    #[test]
    fn same_origin_checks_port() {
        let base = NormalizedBaseUrl::parse("http://127.0.0.1:8000/v1").unwrap();
        let a = reqwest::Url::parse("http://127.0.0.1:8000/v1/models").unwrap();
        let b = reqwest::Url::parse("http://127.0.0.1:9000/v1/models").unwrap();
        assert!(base.same_origin(&a));
        assert!(!base.same_origin(&b));
    }
}
