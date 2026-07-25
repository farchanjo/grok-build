//! Ignored / manual hosted and remote conformance harnesses.
//!
//! These modules never run live traffic unless explicitly enabled by the
//! operator with the documented environment variables. Default CI must not
//! bill or open SSH sessions.

pub mod solaris;
pub mod zai;

pub use solaris::{SOLARIS_HARNESS_ENV, SolarisHarnessConfig, SolarisServiceTarget};
pub use zai::{ZAI_CONFORMANCE_ENV, ZaiConformanceConfig, ZaiConformanceReport};
