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
pub mod store;
pub mod style;
pub mod tokens;

pub use detect::ContentType;
pub use engine::{Engine, Mode, Options, Result as CompressResult};
pub use safety::{Class as SafetyClass, Info as SafetyInfo, lookup};
pub use store::SqliteStore;
pub use tokens::{Counter, TokenEstimate};

/// Build an engine in the given mode with the default compressor set and a
/// persistent store at `grok_home/tersify.db`.
#[must_use]
pub fn default_engine(mode: Mode, grok_home: &std::path::Path) -> Engine {
    let db_path = grok_home.join("tersify.db");
    let store: Option<Box<dyn crate::engine::RecoveryStore>> = match SqliteStore::open(&db_path) {
        Ok(s) => Some(Box::new(s)),
        Err(e) => {
            // An unopenable store must not disable the engine: record mode
            // still works, and compress mode fails closed per payload.
            tracing::warn!(
                path = %db_path.display(),
                error = %e,
                "tersify store unavailable; running without recovery"
            );
            None
        }
    };
    Engine::new(
        mode,
        Box::new(tokens::ApproxCounter),
        vec![Box::new(compressors::log::LogCompressor)],
        store,
    )
}
