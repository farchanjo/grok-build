//! The S0-S4 safety ladder. Every compressor declares its class; the class is
//! inherent to the compression method, never a user choice.
//!
//! The registry is the single place that answers the two honesty questions
//! about a class: does it change model-visible bytes, and does it require a
//! recoverable record before it may run. The exhaustive `match` below makes an
//! unhandled class a compile error rather than a forgotten branch, which is the
//! property this layer exists to provide.

use serde::{Deserialize, Serialize};

/// A position on the safety ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Class {
    /// Byte-safe behavior (metadata, accounting). No model-visible bytes change.
    S0,
    /// Provider-native hints (cache, routing). No model-visible bytes change.
    S1,
    /// Structural changes that need host cooperation.
    S2,
    /// Behavioral changes (routing, reasoning); eval-gated before activation.
    S3,
    /// Lossy structural compression. Alters model-visible bytes, so it is
    /// opt-in, must be reversible through recovery, and must disclose what it
    /// dropped.
    S4,
}

/// The honesty contract of a safety class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    pub class: Class,
    /// True when the class never alters model-visible bytes.
    pub byte_safe: bool,
    /// True when the class is lossy and may only run if the original bytes are
    /// stored for recovery first.
    pub requires_recovery: bool,
    /// True when the model-visible output retains the full value with nothing
    /// dropped. S4 defaults false; a specific method can override it when its
    /// transform is lossless to the model.
    pub reversible: bool,
}

/// Look up a class's contract. Every declared class resolves; there is no
/// unknown-class runtime hole because the language enum is already closed.
#[must_use]
pub const fn lookup(class: Class) -> Info {
    match class {
        Class::S0 => Info {
            class,
            byte_safe: true,
            requires_recovery: false,
            reversible: true,
        },
        Class::S1 => Info {
            class,
            byte_safe: true,
            requires_recovery: false,
            reversible: true,
        },
        Class::S2 => Info {
            class,
            byte_safe: false,
            requires_recovery: false,
            reversible: true,
        },
        Class::S3 => Info {
            class,
            byte_safe: false,
            requires_recovery: false,
            reversible: false,
        },
        Class::S4 => Info {
            class,
            byte_safe: false,
            requires_recovery: true,
            reversible: false,
        },
    }
}

impl Class {
    /// Whether a lossy transform of this class must be stored before it may
    /// emit compressed bytes. Callers treat the answer as a gate, not advice.
    #[must_use]
    pub const fn requires_recovery(self) -> bool {
        lookup(self).requires_recovery
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_lossy_classes_require_recovery() {
        assert!(!lookup(Class::S0).requires_recovery);
        assert!(!lookup(Class::S1).requires_recovery);
        assert!(!lookup(Class::S2).requires_recovery);
        assert!(!lookup(Class::S3).requires_recovery);
        assert!(lookup(Class::S4).requires_recovery);
    }

    #[test]
    fn byte_safe_classes_never_change_model_visible_bytes() {
        for class in [Class::S0, Class::S1] {
            let info = lookup(class);
            assert!(info.byte_safe, "{class:?} must be byte-safe");
            assert!(info.reversible, "{class:?} must be reversible");
        }
        for class in [Class::S2, Class::S3, Class::S4] {
            assert!(
                !lookup(class).byte_safe,
                "{class:?} may change model-visible bytes"
            );
        }
    }

    #[test]
    fn class_order_is_safest_first() {
        assert!(Class::S0 < Class::S4);
    }
}
