//! Compatibility access to provider-neutral extra TLS root support.
//!
//! The implementation lives in `xai-grok-tools`; this module preserves the
//! existing inference path for downstream callers.

pub use xai_grok_inference_types::extra_ca::{
    ENV_GROK_EXTRA_CA_BUNDLE, MAX_EXTRA_CA_BUNDLE_BYTES, extra_root_certificate_der,
    with_extra_root_certificates, with_extra_root_certificates_blocking,
};
