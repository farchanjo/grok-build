//! Generated modules.
pub mod bindings;
pub mod openai_admin_ops;
pub mod openai_admin_types;
pub mod openai_ops;
pub mod openai_types;
pub mod openrouter_ops;
pub mod openrouter_types;
pub use bindings::{
    BINARY_PRIMARY_COUNT, OPENAI_ADMIN_BINDING_COUNT, OPENAI_APP_BINDING_COUNT,
    OPENAI_PRIMARY_COUNT, OPENROUTER_BINDING_COUNT, OPENROUTER_PRIMARY_COUNT, OPERATION_BINDINGS,
    OperationBinding, SSE_COMPANION_COUNT, TOTAL_BINDING_COUNT, find_binding,
    openrouter_path_is_admin, operation_requires_confirmation,
};
