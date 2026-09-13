//! Typed, secret-free asset-store errors.
//!
//! [`AssetError`] is `Debug + Clone + Send + Sync + 'static` on purpose: it
//! crosses the tool boundary, converts into `anyhow::Error`, and carries no
//! `io::Error` (which is not `Clone`) and no signature-bearing URL.
//!
//! `Display` runs every detail through [`redact_detail`], which drops the query
//! string of any embedded URL. A presigned S3 URL therefore prints as
//! `https://bucket.s3.amazonaws.com/k` instead of leaking
//! `X-Amz-Signature=...` into logs and tool output.

use std::fmt;

use super::BackendKind;

/// The store operation an error came from.
///
/// Stable snake_case names feed both [`AssetError::code`] payloads and the
/// capability fast-fail, which needs the operation to build a precise
/// `Unsupported` message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetOperation {
    Put,
    PutFile,
    Get,
    DownloadTo,
    Exists,
    Delete,
    List,
    PresignGet,
    PresignPut,
    SetVisibility,
    Health,
}

impl AssetOperation {
    /// Stable snake_case name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Put => "put",
            Self::PutFile => "put_file",
            Self::Get => "get",
            Self::DownloadTo => "download_to",
            Self::Exists => "exists",
            Self::Delete => "delete",
            Self::List => "list",
            Self::PresignGet => "presign_get",
            Self::PresignPut => "presign_put",
            Self::SetVisibility => "set_visibility",
            Self::Health => "health",
        }
    }
}

impl fmt::Display for AssetOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Every failure mode of the asset subsystem.
///
/// `Unauthorized` (401) and `AccessDenied` (403) stay distinct on purpose: 401
/// feeds the repository's credential-attribution path, 403 maps to a
/// permission error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AssetError {
    /// The object does not exist.
    NotFound { key: String },
    /// 401 — credentials missing, expired, or rejected.
    Unauthorized {
        backend: BackendKind,
        detail: String,
    },
    /// 403 — credentials valid but the operation is not permitted.
    AccessDenied {
        backend: BackendKind,
        detail: String,
    },
    /// The backend cannot perform this operation. Never a fake success.
    Unsupported {
        backend: BackendKind,
        operation: AssetOperation,
        reason: String,
    },
    /// The key is not path-safe.
    InvalidKey { key: String, reason: String },
    /// The content type is not a valid RFC 7231 media type.
    InvalidContentType { value: String, reason: String },
    /// A presign TTL fell outside `[MIN_ASSET_TTL_SECS, MAX_ASSET_TTL_SECS]`.
    TtlOutOfRange { secs: u64, min: u64, max: u64 },
    /// The object exceeds `[assets].max_object_bytes`.
    ObjectTooLarge { key: String, size: u64, max: u64 },
    /// A conditional request lost (`If-Match` / `If-None-Match`).
    PreconditionFailed { key: String, detail: String },
    /// A retryable transport or 5xx failure. The only retryable variant.
    Transient {
        backend: BackendKind,
        detail: String,
    },
    /// A local filesystem failure.
    Io {
        operation: &'static str,
        detail: String,
    },
}

impl AssetError {
    /// Stable snake_case code for tool `details.code` and metrics.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "asset_not_found",
            Self::Unauthorized { .. } => "asset_unauthorized",
            Self::AccessDenied { .. } => "asset_access_denied",
            Self::Unsupported { .. } => "asset_unsupported",
            Self::InvalidKey { .. } => "asset_invalid_key",
            Self::InvalidContentType { .. } => "asset_invalid_content_type",
            Self::TtlOutOfRange { .. } => "asset_ttl_out_of_range",
            Self::ObjectTooLarge { .. } => "asset_object_too_large",
            Self::PreconditionFailed { .. } => "asset_precondition_failed",
            Self::Transient { .. } => "asset_transient",
            Self::Io { .. } => "asset_io",
        }
    }

    /// Whether a plain retry can succeed.
    ///
    /// `Transient` only — retries stay explicit, there is no blanket wrapper.
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Transient { .. })
    }

    /// Build an `Unsupported` error for `operation` on `backend`.
    pub fn unsupported(
        backend: BackendKind,
        operation: AssetOperation,
        reason: impl Into<String>,
    ) -> Self {
        Self::Unsupported {
            backend,
            operation,
            reason: reason.into(),
        }
    }

    /// Build an `Io` error, keeping the message but dropping the non-`Clone`
    /// `io::Error` itself.
    pub fn io(operation: &'static str, err: &std::io::Error) -> Self {
        Self::Io {
            operation,
            detail: err.to_string(),
        }
    }

    /// Backend this error came from, when it is backend-attributable.
    pub const fn backend(&self) -> Option<BackendKind> {
        match self {
            Self::Unauthorized { backend, .. }
            | Self::AccessDenied { backend, .. }
            | Self::Unsupported { backend, .. }
            | Self::Transient { backend, .. } => Some(*backend),
            _ => None,
        }
    }

    /// The 401/403 signal in the shape the caller needs to branch on.
    pub const fn is_auth_failure(&self) -> bool {
        matches!(self, Self::Unauthorized { .. })
    }
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { key } => write!(f, "asset `{key}` not found"),
            Self::Unauthorized { backend, detail } => write!(
                f,
                "{} rejected the credentials: {}",
                backend,
                redact_detail(detail)
            ),
            Self::AccessDenied { backend, detail } => {
                write!(f, "{} denied access: {}", backend, redact_detail(detail))
            }
            Self::Unsupported {
                backend,
                operation,
                reason,
            } => write!(
                f,
                "{} does not support {operation}: {}",
                backend,
                redact_detail(reason)
            ),
            Self::InvalidKey { key, reason } => write!(f, "invalid asset key `{key}`: {reason}"),
            Self::InvalidContentType { value, reason } => {
                write!(f, "invalid content type `{value}`: {reason}")
            }
            Self::TtlOutOfRange { secs, min, max } => {
                write!(f, "presign TTL {secs}s is outside {min}..={max}s")
            }
            Self::ObjectTooLarge { key, size, max } => {
                write!(
                    f,
                    "asset `{key}` is {size} bytes, over the {max}-byte limit"
                )
            }
            Self::PreconditionFailed { key, detail } => write!(
                f,
                "precondition failed for asset `{key}`: {}",
                redact_detail(detail)
            ),
            Self::Transient { backend, detail } => {
                write!(f, "{backend} transient failure: {}", redact_detail(detail))
            }
            Self::Io { operation, detail } => {
                write!(f, "{operation} failed: {}", redact_detail(detail))
            }
        }
    }
}

impl std::error::Error for AssetError {}

/// Strip the query string (and fragment) from every URL in `detail`.
///
/// Signature-bearing presign parameters live in the query string, so dropping
/// it is enough to keep `Display` secret-free without guessing parameter names.
pub(crate) fn redact_detail(detail: &str) -> String {
    let mut out = String::with_capacity(detail.len());
    let mut rest = detail;
    while !rest.is_empty() {
        let url_len = if rest.starts_with("https://") {
            Some(8)
        } else if rest.starts_with("http://") {
            Some(7)
        } else {
            None
        };
        match url_len {
            Some(_) => {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                let url = &rest[..end];
                let cut = url.find(['?', '#']).unwrap_or(url.len());
                out.push_str(&url[..cut]);
                rest = &rest[end..];
            }
            None => {
                let ch = rest.chars().next().expect("non-empty");
                out.push(ch);
                rest = &rest[ch.len_utf8()..];
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::key::{ContentTypeError, KeyError};

    #[test]
    fn codes_are_stable_and_unique() {
        let samples = [
            AssetError::NotFound { key: "k".into() },
            AssetError::Unauthorized {
                backend: BackendKind::S3,
                detail: "d".into(),
            },
            AssetError::AccessDenied {
                backend: BackendKind::S3,
                detail: "d".into(),
            },
            AssetError::unsupported(BackendKind::Local, AssetOperation::PresignPut, "r"),
            AssetError::InvalidKey {
                key: "k".into(),
                reason: "r".into(),
            },
            AssetError::InvalidContentType {
                value: "v".into(),
                reason: "r".into(),
            },
            AssetError::TtlOutOfRange {
                secs: 0,
                min: 1,
                max: 10,
            },
            AssetError::ObjectTooLarge {
                key: "k".into(),
                size: 2,
                max: 1,
            },
            AssetError::PreconditionFailed {
                key: "k".into(),
                detail: "d".into(),
            },
            AssetError::Transient {
                backend: BackendKind::Gcs,
                detail: "d".into(),
            },
            AssetError::Io {
                operation: "read",
                detail: "d".into(),
            },
        ];
        let mut codes: Vec<&str> = samples.iter().map(|e| e.code()).collect();
        codes.sort_unstable();
        let before = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), before, "codes must be unique: {codes:?}");
        for code in codes {
            assert!(code.starts_with("asset_"), "{code}");
            assert!(
                code.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{code}"
            );
        }
        assert_eq!(
            AssetError::NotFound { key: "k".into() }.code(),
            "asset_not_found"
        );
    }

    #[test]
    fn only_transient_is_retryable() {
        assert!(
            AssetError::Transient {
                backend: BackendKind::S3,
                detail: "503".into()
            }
            .is_retryable()
        );
        for err in [
            AssetError::NotFound { key: "k".into() },
            AssetError::Unauthorized {
                backend: BackendKind::S3,
                detail: "401".into(),
            },
            AssetError::AccessDenied {
                backend: BackendKind::S3,
                detail: "403".into(),
            },
            AssetError::unsupported(BackendKind::Proxy, AssetOperation::Delete, "r"),
            AssetError::TtlOutOfRange {
                secs: 0,
                min: 1,
                max: 10,
            },
            AssetError::Io {
                operation: "read",
                detail: "d".into(),
            },
        ] {
            assert!(!err.is_retryable(), "{err:?} must not be retryable");
        }
    }

    #[test]
    fn unauthorized_and_access_denied_stay_distinct() {
        let unauthorized = AssetError::Unauthorized {
            backend: BackendKind::S3,
            detail: "401".into(),
        };
        let denied = AssetError::AccessDenied {
            backend: BackendKind::S3,
            detail: "403".into(),
        };
        assert!(unauthorized.is_auth_failure());
        assert!(!denied.is_auth_failure());
        assert_ne!(unauthorized.code(), denied.code());
    }

    #[test]
    fn display_is_secret_free() {
        let signed = AssetError::Transient {
            backend: BackendKind::S3,
            detail: "GET https://bucket.s3.amazonaws.com/uploads/a.png\
                     ?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=deadbeef failed"
                .into(),
        };
        let rendered = signed.to_string();
        assert!(!rendered.contains("X-Amz-Signature"), "{rendered}");
        assert!(!rendered.contains('?'), "{rendered}");
        assert!(rendered.contains("https://bucket.s3.amazonaws.com/uploads/a.png"));
        assert!(rendered.contains("failed"));
        assert!(rendered.starts_with("s3 transient failure:"));

        let unauthorized = AssetError::Unauthorized {
            backend: BackendKind::Gcs,
            detail: "https://storage.googleapis.com/b/o?sk-signature=1".into(),
        };
        let rendered = unauthorized.to_string();
        assert!(!rendered.contains("sk-signature"), "{rendered}");
        assert!(!rendered.contains("sk-"), "{rendered}");
    }

    #[test]
    fn redact_detail_keeps_non_url_text_verbatim() {
        assert_eq!(redact_detail("plain message"), "plain message");
        assert_eq!(
            redact_detail("see https://x.test/a?b=c now"),
            "see https://x.test/a now"
        );
        assert_eq!(redact_detail("https://x.test/a#frag"), "https://x.test/a");
        assert_eq!(redact_detail(""), "");
    }

    #[test]
    fn key_and_content_type_errors_convert() {
        let from_key: AssetError = KeyError::Empty.into();
        assert_eq!(from_key.code(), "asset_invalid_key");
        let from_ct: AssetError = ContentTypeError::Empty.into();
        assert_eq!(from_ct.code(), "asset_invalid_content_type");
    }

    #[test]
    fn backend_attribution_is_reported() {
        assert_eq!(
            AssetError::Transient {
                backend: BackendKind::S3,
                detail: String::new()
            }
            .backend(),
            Some(BackendKind::S3)
        );
        assert_eq!(AssetError::NotFound { key: "k".into() }.backend(), None);
    }

    /// The error must cross the tool boundary and convert into `anyhow::Error`,
    /// which requires an owned, thread-safe, `'static` error.
    #[test]
    fn asset_error_is_send_sync_clone_static_and_std_error() {
        fn assert_bounds<T: std::error::Error + Clone + Send + Sync + 'static>() {}
        assert_bounds::<AssetError>();

        let err = AssetError::NotFound { key: "k".into() };
        let boxed: anyhow::Error = err.clone().into();
        assert_eq!(boxed.to_string(), err.to_string());
    }
}
