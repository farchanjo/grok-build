//! Wire dialect for OpenAI-compatible backends.
//!
//! A single [`ProviderKind::OpenAiCompatible`] adapter serves several
//! OpenAI-compatible wire dialects (vLLM, SGLang, and the legacy
//! "standard" OpenAI-compatible shape). The dialect is config data carried
//! by the adapter — it tunes request-side reasoning keys, the reasoning
//! echo policy, and the per-delta [`shape_delta`](super::ProviderAdapter::shape_delta)
//! hook.
//!
//! Unknown dialect strings **fail closed** at the TOML boundary:
//! [`WireDialect::parse`] returns `None` for any unrecognized value, so a
//! typo'd `dialect = "vlllm"` never silently selects the standard wire and
//! never reaches a backend. (The `#[non_exhaustive]` attribute keeps the
//! enum open for future dialects so downstream crates cannot match it
//! exhaustively.)

/// Wire dialect of an OpenAI-compatible backend.
///
/// `#[non_exhaustive]`: adding a dialect is a non-breaking change, so
/// downstream crates must not match on this enum without a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WireDialect {
    /// The canonical OpenAI-compatible chat wire (legacy default). Reasoning
    /// is carried in `delta.reasoning_content` and echoed verbatim on replay.
    Standard,
    /// vLLM server. The wire may emit reasoning under the `reasoning` key and
    /// the adapter hoists reasoning out of `content`; replayed assistant
    /// reasoning is stripped (context-budget protection).
    Vllm,
    /// SGLang server. Treated like vLLM for reasoning key / echo purposes.
    Sglang,
}

impl WireDialect {
    /// Canonical TOML/string identifier for this dialect.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Vllm => "vllm",
            Self::Sglang => "sglang",
        }
    }

    /// Fail-closed parsing of a TOML/string dialect value.
    ///
    /// Returns `None` for any unrecognized string so a malformed `dialect`
    /// field can never silently fall back to the standard wire.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "standard" => Some(Self::Standard),
            "vllm" => Some(Self::Vllm),
            "sglang" => Some(Self::Sglang),
            _ => None,
        }
    }

    /// Whether reasoning is expected under the vLLM/SGLang `reasoning` key
    /// (as opposed to OpenAI-compatible `reasoning_content`).
    pub const fn uses_reasoning_key(self) -> bool {
        matches!(self, Self::Vllm | Self::Sglang)
    }

    /// Whether replayed assistant reasoning should be stripped.
    pub const fn strips_reasoning_echo(self) -> bool {
        matches!(self, Self::Vllm | Self::Sglang)
    }
}

impl Default for WireDialect {
    fn default() -> Self {
        Self::Standard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_known_dialects() {
        assert_eq!(WireDialect::parse("standard"), Some(WireDialect::Standard));
        assert_eq!(WireDialect::parse("vllm"), Some(WireDialect::Vllm));
        assert_eq!(WireDialect::parse("sglang"), Some(WireDialect::Sglang));
    }

    #[test]
    fn parse_unknown_fails_closed() {
        assert_eq!(WireDialect::parse("vlllm"), None);
        assert_eq!(WireDialect::parse(""), None);
        assert_eq!(WireDialect::parse("Standard"), None, "case-sensitive");
        assert_eq!(WireDialect::parse("azure"), None);
    }

    #[test]
    fn as_str_round_trips_via_parse() {
        for dialect in [
            WireDialect::Standard,
            WireDialect::Vllm,
            WireDialect::Sglang,
        ] {
            assert_eq!(WireDialect::parse(dialect.as_str()), Some(dialect));
        }
    }

    #[test]
    fn vllm_family_uses_reasoning_key_and_strips_echo() {
        assert!(WireDialect::Vllm.uses_reasoning_key());
        assert!(WireDialect::Sglang.uses_reasoning_key());
        assert!(WireDialect::Vllm.strips_reasoning_echo());
        assert!(WireDialect::Sglang.strips_reasoning_echo());
        assert!(!WireDialect::Standard.uses_reasoning_key());
        assert!(!WireDialect::Standard.strips_reasoning_echo());
    }

    #[test]
    fn serde_round_trips_snake_case_and_rejects_unknown() {
        for dialect in [
            WireDialect::Standard,
            WireDialect::Vllm,
            WireDialect::Sglang,
        ] {
            let json = serde_json::to_string(&dialect).expect("serialize dialect");
            assert_eq!(json, format!("\"{}\"", dialect.as_str()));
            let back: WireDialect = serde_json::from_str(&json).expect("deserialize dialect");
            assert_eq!(back, dialect);
        }
        // Fail closed on an unknown string.
        assert!(serde_json::from_str::<WireDialect>("\"vlllm\"").is_err());
    }
}
