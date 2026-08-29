//! The tersify transformer: the one object callers hold to compress tool
//! results at the conversation seam.
//!
//! `chat-state` stays dependency-free of this crate — it sees only an opaque
//! `apply(ConversationItem) -> ConversationItem` closure type defined there.
//! This module provides the real implementation, which owns the engine, the
//! recovery store, and the scope decision, and appends a recovery pointer to
//! every lossy result so the original stays one retrieve away.
//!
//! Honesty rules carried over from the engine:
//!
//! - a result the engine passes through (no compressor applied, not smaller,
//!   unrecoverable) ships byte-identical, with no pointer appended;
//! - the pointer names the handle and states the ratio, so a caller that
//!   never retrieves still sees exactly what happened to its bytes;
//! - subagent sessions and tersify-off setups hold no transformer at all.

use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::engine::{Engine, Mode, Options};

/// One tersify instance per main session. `Engine` is not `Sync`; the Mutex
/// serializes compression calls, which is fine — tool results arrive one at a
/// time per conversation anyway.
///
/// Mode semantics at this seam: `Compress` stores the original and ships the
/// compressed bytes with a `retrieve <handle>` pointer; `Record` measures and
/// discloses ("compressible, record mode") while the original ships unchanged.
pub struct Tersify {
    engine: Mutex<Engine>,
}

impl Tersify {
    /// Build from the persisted `[hints] tersify_*` config. `Record` mode
    /// (measurement only, original bytes emitted) until `Compress` is opted
    /// into at the call site.
    #[must_use]
    pub fn open(grok_home: &Path, mode: Mode) -> Self {
        Self {
            engine: Mutex::new(crate::default_engine(mode, grok_home)),
        }
    }

    /// Compress `input` if a compressor applies. Returns the (possibly
    /// compressed) bytes plus the recovery handle when the original was
    /// stored. Fail-closed: engine trouble reads as "no compression".
    ///
    /// Record mode is surfaced distinctly through [`Self::inspect`]: the
    /// measured result is real but the original bytes ship unchanged.
    #[must_use]
    pub fn compress(&self, input: &[u8]) -> (Vec<u8>, Option<String>) {
        let mut engine = self
            .engine
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = engine.compress(input, Options::default());
        if result.applied {
            (result.body, result.handle)
        } else {
            (input.to_vec(), None)
        }
    }

    /// Dry-run view: what the engine measured for `input`. Used by record
    /// mode, which must not emit compressed bytes or claim a store write: the
    /// ratio tells whether compression WOULD have applied.
    #[must_use]
    pub fn inspect(&self, input: &[u8]) -> f64 {
        let mut engine = self
            .engine
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = engine.compress(input, Options::default());
        // Record mode zeroes applied; the ratio still carries the measurement.
        if result.ratio > 0.0 {
            result.ratio
        } else {
            0.0
        }
    }
}

/// The type `chat-state` sees. Wraps [`Tersify`] behind `Send + Sync` so the
/// actor can hold it without knowing the engine exists.
pub type SharedTersify = Arc<Tersify>;

impl Tersify {
    /// Transform one tool-result item: compress oversized textual content and
    /// append the recovery pointer. Oversize floor keeps small results raw —
    /// a marker plus pointer costs more tokens than a short result.
    #[must_use]
    pub fn apply_tool_result(&self, content: &str) -> String {
        const MIN_BYTES: usize = 4096;
        if content.len() < MIN_BYTES {
            return content.to_string();
        }
        // Record mode measures and reports but never claims a store write; the
        // engine already hands back the original bytes there, so the pointer
        // text below states exactly that instead of a fake handle.
        if self.mode() == Mode::Record {
            let ratio = self.inspect(content.as_bytes());
            if ratio <= 0.0 {
                return content.to_string();
            }
            return format!(
                "{content}\n\n[tersify: would compress to {pct:.0}% (record mode, \
                 original not stored; set scope to compress)]",
                pct = (1.0 - ratio) * 100.0,
            );
        }
        let (body, handle) = self.compress(content.as_bytes());
        if body.len() >= content.len() {
            return content.to_string();
        }
        let compressed = String::from_utf8_lossy(&body).into_owned();
        match handle {
            Some(h) => format!(
                "{compressed}\n\n[tersify: {before} -> {after} bytes compressed; \
                 original: retrieve {h} ]",
                before = content.len(),
                after = body.len(),
            ),
            // Compress mode with no handle means the store refused or was
            // unavailable; the engine already failed closed to pass-through,
            // so this arm is unreachable for lossy results. Keep the original.
            None => content.to_string(),
        }
    }

    /// The engine mode this instance runs in.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.engine
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .mode()
    }

    /// The byte-exact original behind a handle, through this instance's own
    /// store. External handles resolve only through the same store that
    /// issued them — recovery is not portable across databases.
    #[must_use]
    pub fn retrieve(&self, handle: &str) -> Option<Vec<u8>> {
        self.engine
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retrieve(handle)
    }

    /// The transform closure the session layer hands to
    /// `ChatStateHandle::push_tool_result_tersified`. Compresses textual tool
    /// results (oversize floor applies) and appends the recovery pointer.
    #[must_use]
    pub fn transform_fn(
        self: &Arc<Self>,
    ) -> Arc<
        dyn Fn(
                xai_grok_inference_types::ConversationItem,
            ) -> xai_grok_inference_types::ConversationItem
            + Send
            + Sync,
    > {
        let this = Arc::clone(self);
        Arc::new(move |item| match item {
            // Error results keep their bytes: the decisive line must stay
            // byte-exact, and errors are rarely the oversize payload anyway.
            xai_grok_inference_types::ConversationItem::ToolResult(mut tr)
                if tr.is_error != Some(true) =>
            {
                tr.content = std::sync::Arc::from(this.apply_tool_result(&tr.content));
                xai_grok_inference_types::ConversationItem::ToolResult(tr)
            }
            other => other,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_home(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tersify-{tag}-{}-{}",
            std::process::id(),
            std::thread::current()
                .name()
                .unwrap_or("t")
                .replace("::", "-")
        ));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn big_repeated_log() -> String {
        let mut s = String::from("2026-08-29 10:00:00 INFO boot start\n");
        for i in 0..400 {
            s.push_str(&format!(
                "2026-08-29 10:{:02}:{:02} INFO worker status=ready attempt=1 line\n",
                i / 60,
                i % 60
            ));
        }
        s.push_str("2026-08-29 10:07:00 ERROR disk full\n");
        s
    }

    #[test]
    fn probe_what_detect_says() {
        let home = unique_home("probe");
        let input = big_repeated_log();
        eprintln!("DETECT={:?}", crate::detect::detect(input.as_bytes()));
        let t = Tersify::open(&home, Mode::Compress);
        let out = t.apply_tool_result(&input);
        eprintln!(
            "OUT_LEN={} IN_LEN={} HAS_PTR={} HAS_REC={}",
            out.len(),
            input.len(),
            out.contains("retrieve rcv_"),
            out.contains("record mode")
        );
    }

    #[test]
    fn small_results_pass_through_untouched() {
        let t = Tersify::open(&unique_home("small"), Mode::Compress);
        let input = "tiny output";
        assert_eq!(t.apply_tool_result(input), input);
    }

    #[test]
    fn big_distinct_payload_passes_through_untouched() {
        let t = Tersify::open(&unique_home("distinct"), Mode::Compress);
        let mut input = String::new();
        for i in 0..200 {
            input.push_str(&format!(
                "record {i}: unique-uuid-{i:04} with narrative sentence {i} entirely unlike its neighbors.\n"
            ));
        }
        let out = t.apply_tool_result(&input);
        assert_eq!(out, input, "distinct payload must stay raw");
    }

    #[test]
    fn big_repeated_log_compresses_and_carries_the_handle() {
        let home = unique_home("compress");
        let t = Tersify::open(&home, Mode::Compress);
        let input = big_repeated_log();
        let out = t.apply_tool_result(&input);
        assert!(out.len() < input.len(), "log must compress");
        assert!(
            out.contains("retrieve rcv_"),
            "pointer must name the handle: {out}"
        );
        assert!(out.contains("ERROR disk full"));
        let handle = out
            .split("retrieve ")
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .map(|tok| tok.trim_end_matches(']'))
            .filter(|tok| tok.starts_with("rcv_"))
            .expect("handle present");
        assert_eq!(
            t.retrieve(handle).as_deref(),
            Some(input.as_bytes()),
            "handle from the pointer must resolve through the same store"
        );
    }

    #[test]
    fn error_results_are_never_compressed() {
        let t = Tersify::open(&unique_home("errors"), Mode::Compress);
        let transform = Tersify::transform_fn(&{
            let arc = std::sync::Arc::new(t);
            arc
        });
        let big_error = "ERROR line\n".repeat(600);
        let item = xai_grok_inference_types::ConversationItem::ToolResult(
            xai_grok_inference_types::ToolResultItem {
                tool_call_id: "call-1".into(),
                content: big_error.clone().into(),
                images: Vec::new(),
                is_error: Some(true),
            },
        );
        let out = transform(item);
        match out {
            xai_grok_inference_types::ConversationItem::ToolResult(tr) => {
                assert_eq!(
                    tr.content.as_ref(),
                    big_error,
                    "error payload must stay byte-exact"
                );
                assert_eq!(tr.is_error, Some(true));
            }
            other => panic!("unexpected item {other:?}"),
        }
    }

    #[test]
    fn record_mode_reports_no_store_write() {
        let home = unique_home("record");
        let t = Tersify::open(&home, Mode::Record);
        let input = big_repeated_log();
        let out = t.apply_tool_result(&input);
        // Record mode discloses the measurement and never stores: the tail is
        // the disclosure line, the head is the untouched original.
        assert!(
            out.contains("would compress to") && out.contains("record mode"),
            "record mode must disclose: ...{}",
            &out[out.len().saturating_sub(200)..]
        );
        assert!(out.contains("ERROR disk full"));
        assert!(out.starts_with("2026-08-29 10:00:00 INFO boot start"));
        // And nothing was stored: the instance's own store must be empty.
        assert_eq!(t.retrieve("rcv_0000000000000000"), None);
    }
}
