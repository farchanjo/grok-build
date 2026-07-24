//! Per-response inference latency metrics.
//!
//! This module is a thin re-export of the sampler crate's canonical
//! `InferenceLatencyStats` and `compute_percentiles` helpers. It
//! preserves the import path
//! `crate::session::inference_metrics::InferenceLatencyStats` for any
//! call-sites that still spell it that way.

pub use xai_grok_inference::{InferenceLatencyStats, compute_percentiles};
