//! Callable subagent discovery snapshots (PR20).
//!
//! [`callable`] exposes an authoritative, credential/body-free
//! [`callable::CallableAgentDescriptor`] snapshot used by the shell's agent
//! recommendation pipeline. It is driven off the SAME discovery, shadowing,
//! trust/plugin source, toggle, and qualified-name resolution that the Task
//! tool and spawn path use — it never re-implements or approximates that
//! precedence.

pub mod callable;
