//! Auth dependency-inversion seam shared between `xai-file-utils`
//! (the holder) and `xai-grok-shell` (the implementer). Keeps shell types
//! out of data-collector's import graph while still letting refresh-aware
//! token resolution drive HTTP requests.

pub mod auth_provider;
pub mod bearer_fragment;
#[cfg(feature = "middleware")]
pub mod retry_middleware;
pub mod visibility;

pub use auth_provider::{AuthCredentialProvider, CredentialSnapshot, StaticAuthCredentialProvider};
pub use bearer_fragment::{BEARER_TAIL_CHARS, bearer_tail};
#[cfg(feature = "middleware")]
pub use retry_middleware::{AuthRetryMiddleware, StampedBearerTail};
pub use visibility::HttpAuth;
