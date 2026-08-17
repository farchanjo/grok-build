//! Shared utilities used by both `xai-grok-shell` and its downstream clients
//! (e.g. `xai-grok-pager-render`). This crate sits upstream of `xai-grok-shell`
//! so it must never depend on it.

pub mod clipboard;
pub mod extra_ca {
    //! Extra TLS root support exposed through this crate's existing tools edge.
    pub use xai_grok_tools::extra_ca::*;
}
pub mod placeholder_images;
pub mod session;
pub mod stderr;
pub mod ui_config;
