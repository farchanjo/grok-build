//! Token-economy engine for the main conversation context.
//!
//! The engine implements the stable four-call surface shared by every consumer:
//!
//! ```text
//! compress  -> detect content type -> route to a safety-classed compressor
//!             -> measure reduction -> store the original for recovery
//! retrieve  -> the byte-exact original for a stored handle
//! detect    -> classify a payload (low confidence falls open to `Text`)
//! stats     -> local counters; every number is an estimate, never provider-billed
//! ```
//!
//! Two rules govern every path through this crate and are treated as
//! correctness, not tuning:
//!
//! 1. **Fail closed.** When nothing applies, the original bytes pass through
//!    unchanged with a zero ratio. Compression never errors and never truncates
//!    a document to look smaller.
//! 2. **Estimates are disclosed as estimates.** Token counts are local BPE
//!    approximations; provider usage is authoritative downstream.
//!
//! Scope policy — which conversations see compressed style at all — lives with
//! the caller. This crate answers "how do bytes shrink and come back", not
//! "who may be compressed".

pub mod compressors;
pub mod detect;
pub mod engine;
pub mod safety;
pub mod tokens;

pub use detect::ContentType;
pub use engine::{Engine, Mode, Options, Result as CompressResult};
pub use safety::{Class as SafetyClass, Info as SafetyInfo, lookup};
pub use tokens::{Counter, TokenEstimate};
