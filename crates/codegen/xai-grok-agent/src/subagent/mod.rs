//! Callable subagent discovery snapshots (PR20).
//!
//! [`callable`] exposes an authoritative, credential/body-free
//! [`callable::CallableAgentDescriptor`] snapshot used by the shell's agent
//! recommendation pipeline. It is driven off the SAME discovery, shadowing,
//! trust/plugin source, toggle, and qualified-name resolution that the Task
//! tool and spawn path use — it never re-implements or approximates that
//! precedence. Plugin agents are toggle-filtered by qualified name (M1) and
//! keyed by filename (not frontmatter `name`, M4), CLI-inline agents carry a
//! distinct [`callable::CallableAgentSource::CliInline`] source, and the
//! snapshot is injective on canonical names so ids/ranks/render stay
//! one-to-one.

pub mod callable;
