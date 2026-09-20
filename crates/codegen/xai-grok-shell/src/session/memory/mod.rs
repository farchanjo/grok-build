//! Memory system shim.
//!
//! The memory "core engine" now lives in the standalone `xai-grok-memory`
//! crate. This module re-exports that crate's public surface under the
//! historical `crate::session::memory::*` paths so the ~30 reverse-dependency
//! call sites in this crate keep compiling unchanged.
//!
//! Only `hooks` and `gate` stay here: both are session glue (they depend on
//! `crate::inference`, `crate::auth` and `crate::config`) and are not part of
//! the relocatable core engine.

pub mod gate;
pub mod hooks;

pub use xai_grok_memory::{
    Candidate, DecisionClient, DropReason, EndpointScopedCredentials, GateConfig, GateError,
    GateOutcome, JevDecisionsClient, LONG_TERM_END_MARKER, MemoryBackendImpl, MemoryBackendParams,
    MemoryGate, MemoryIndex, MemoryScope, MemoryStorage, Store, WriteScope, archive, backend,
    chunker, dream, dream_lock, embed_missing_chunks, embed_missing_chunks_with_mirror, embedding,
    gate_client, index, init_sqlite_vec, markdown_sections, mmr, note_scope, query_expansion,
    retrieval, schema, search, storage, text_utils, watcher,
};
