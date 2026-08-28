//! Compressors: pure byte transforms, one module per content type.
//!
//! A compressor never counts tokens, touches recovery storage, or performs I/O
//! — the engine core wraps those around it. That boundary is what keeps each
//! compressor a self-contained, testable file and is why the safety invariants
//! below hold by construction rather than by review.
//!
//! Every compressor is deterministic, idempotent, and fail-closed: on any parse
//! problem it returns `ok = false` and the caller must forward the original
//! bytes unchanged.

pub mod elision;
pub mod log;
pub mod marker;

use crate::safety::Class;

/// Compresses one content type.
pub trait Compressor {
    /// Registry key of the content type this compressor handles.
    fn content_type(&self) -> &'static str;

    /// Inherent safety class of the method, not a user choice.
    fn safety_class(&self) -> Class;

    /// Returns compressed bytes with `ok = true` on success. On any parse
    /// problem, returns `(Vec::new(), false)` and the caller forwards the
    /// original unchanged.
    fn compress(&self, input: &[u8]) -> (Vec<u8>, bool);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety;

    /// A compressor whose class has no recovery contract cannot emit lossy
    /// output; this pins the relationship the engine enforces.
    struct AlwaysFails;

    impl Compressor for AlwaysFails {
        fn content_type(&self) -> &'static str {
            "text"
        }
        fn safety_class(&self) -> Class {
            Class::S4
        }
        fn compress(&self, _input: &[u8]) -> (Vec<u8>, bool) {
            (Vec::new(), false)
        }
    }

    #[test]
    fn fail_closed_compressor_never_claims_a_result() {
        let c = AlwaysFails;
        let (out, ok) = c.compress(b"anything");
        assert!(!ok);
        assert!(out.is_empty());
        assert!(safety::lookup(c.safety_class()).requires_recovery);
    }
}
