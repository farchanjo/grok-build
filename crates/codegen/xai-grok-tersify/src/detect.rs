//! Content detection: classify a payload so the router can pick a compressor.
//!
//! Detection is deterministic and fail-open: anything the rules are not
//! confident about is `Text`, which routes to the most conservative compressor.
//! The check order is strict JSON, terminal, diff, code, log, search-result,
//! then fall-open. Order matters: the terminal signal (a raw ANSI escape) is
//! conclusive and must not be stolen by the looser log or code patterns, and
//! valid JSON must not be read as a config file.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// Content type names, used as registry keys by the compressors module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContentType {
    Json,
    Terminal,
    Diff,
    Code,
    Log,
    SearchResult,
    Html,
    Tabular,
    Config,
    Text,
}

impl ContentType {
    /// Stable registry name. Persisted in recovery records; never rename a
    /// variant's string without migrating stored rows.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ContentType::Json => "json",
            ContentType::Terminal => "terminal",
            ContentType::Diff => "diff",
            ContentType::Code => "code",
            ContentType::Log => "log",
            ContentType::SearchResult => "search-result",
            ContentType::Html => "html",
            ContentType::Tabular => "tabular",
            ContentType::Config => "config",
            ContentType::Text => "text",
        }
    }
}

impl core::fmt::Display for ContentType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

static LOG_LINE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"(?i)(\b(TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL|PANIC)\b|^\s*\[[A-Z]+\]|\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}|\b\d{2}:\d{2}:\d{2}\b)",
    )
    .expect("static log regex")
});

static DIFF_LINE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?m)^(diff --git |@@ |--- |\+\+\+ |[+-][^+-])").expect("static diff regex")
});

static SEARCH_LINE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?m)^([./~A-Za-z0-9_-][^:\n]{0,240}:\d+(:\d+)?:|https?://\S+)")
        .expect("static search regex")
});

static CODE_KEYWORD: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"\b(func|package|import|def|class|function|return|const|let|var|public|private|protected|static|void|struct|interface|namespace|module|fn|impl|trait|export|async|await)\b",
    )
    .expect("static code keyword regex")
});

/// A raw ANSI/CSI escape is the conclusive terminal signal — nothing but
/// terminal or command output legitimately embeds one.
static ANSI_ESCAPE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]").expect("static ansi regex"));

/// Classify `input`. Low confidence always falls open to [`ContentType::Text`].
#[must_use]
pub fn detect(input: &[u8]) -> ContentType {
    let trimmed = trim_ascii(input);
    if trimmed.is_empty() {
        return ContentType::Text;
    }
    // The rules are textual; a payload that is not valid UTF-8 carries no
    // keyword, log level, or search-path signal, and falls open to `Text`.
    let Some(text) = std::str::from_utf8(trimmed).ok() else {
        return ContentType::Text;
    };

    // Strict JSON first: a valid JSON document that also looks like config or
    // terminal output is still JSON, and its compressor is structure-aware.
    if (trimmed[0] == b'{' || trimmed[0] == b'[')
        && serde_json::from_str::<serde::de::IgnoredAny>(text).is_ok()
    {
        return ContentType::Json;
    }

    if ANSI_ESCAPE.is_match(text) {
        return ContentType::Terminal;
    }

    if looks_like_diff(text) {
        return ContentType::Diff;
    }

    if looks_like_code(text) {
        return ContentType::Code;
    }

    if looks_like_log(text) {
        return ContentType::Log;
    }

    if looks_like_search(text) {
        return ContentType::SearchResult;
    }

    ContentType::Text
}

fn trim_ascii(input: &[u8]) -> &[u8] {
    let start = input
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(input.len());
    let end = input
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map_or(start, |i| i + 1);
    &input[start..end]
}

fn looks_like_diff(text: &str) -> bool {
    DIFF_LINE.find_iter(text).count() >= 2
}

fn looks_like_code(text: &str) -> bool {
    CODE_KEYWORD.find_iter(text).count() >= 3
}

fn looks_like_log(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().take(24).collect();
    let hits = lines.iter().filter(|l| LOG_LINE.is_match(l)).count();
    // Most lines carrying a level token or timestamp, and enough lines for the
    // ratio to mean anything: one timestamped line in an essay is a quote, not
    // a log.
    lines.len() >= 4 && hits * 2 >= lines.len()
}

fn looks_like_search(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).take(24).collect();
    let hits = lines.iter().filter(|l| SEARCH_LINE.is_match(l)).count();
    lines.len() >= 3 && hits * 2 >= lines.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_strict_json_before_anything_else() {
        assert_eq!(detect(br#"{"status": "ok"}"#), ContentType::Json);
        assert_eq!(detect(b"[1, 2, 3]"), ContentType::Json);
        // Leading whitespace does not change the class.
        assert_eq!(detect(b"\n  {\"a\": 1}\n"), ContentType::Json);
    }

    #[test]
    fn malformed_json_falls_through_instead_of_erroring() {
        // A trailing comma is not valid JSON; classifying it as text routes it
        // to the conservative compressor instead of a structural one that would
        // fail on it anyway.
        assert_ne!(detect(br#"{"a": 1,}"#), ContentType::Json);
    }

    #[test]
    fn ansi_escape_is_conclusively_terminal() {
        let payload =
            b"total 8\r\ndrwxr-xr-x  2 u g  64 Aug 28 13:00 .\x1b[0m\x1b[38;5;33mfoo\x1b[0m\n";
        assert_eq!(detect(payload), ContentType::Terminal);
    }

    #[test]
    fn detects_git_diff() {
        let payload =
            b"diff --git a/x.rs b/x.rs\n--- a/x.rs\n+++ b/x.rs\n@@ -1,3 +1,3 @@\n-old\n+new\n";
        assert_eq!(detect(payload), ContentType::Diff);
    }

    #[test]
    fn detects_source_code() {
        let payload = b"fn main() {\n    let x = compute();\n    return x;\n}\n";
        assert_eq!(detect(payload), ContentType::Code);
    }

    #[test]
    fn detects_level_or_timestamp_lines_as_log() {
        let payload = b"2026-08-28 13:00:00 INFO boot\n2026-08-28 13:00:01 WARN slow disk\n2026-08-28 13:00:02 INFO ready\n2026-08-28 13:00:03 ERROR crash\n";
        assert_eq!(detect(payload), ContentType::Log);

        let leveled = b"ERROR bad thing\nINFO ok\nERROR other\nWARN careful\n";
        assert_eq!(detect(leveled), ContentType::Log);
    }

    #[test]
    fn prose_with_one_timestamp_is_not_a_log() {
        let payload = b"I remember the meeting on 2026-01-01 10:00:00 clearly.\nIt was a normal day with ordinary sentences.\nNothing about this paragraph is machine output.\nBut it does have four lines to test the ratio.\n";
        assert_eq!(detect(payload), ContentType::Text);
    }

    #[test]
    fn detects_path_colon_line_search_output() {
        let payload = b"src/main.rs:12:fn main() {\nsrc/lib.rs:4:pub mod x;\nsrc/other.rs:99:const Y: u8 = 1;\n";
        assert_eq!(detect(payload), ContentType::SearchResult);
    }

    #[test]
    fn empty_input_is_text() {
        assert_eq!(detect(b""), ContentType::Text);
        assert_eq!(detect(b"   \n\t "), ContentType::Text);
    }

    #[test]
    fn detection_never_panics_on_binary_noise() {
        let payload = vec![0u8, 1, 2, 0x1b, b'[', 255, 128, b'{', b'['];
        let _ = detect(&payload);
    }

    #[test]
    fn registry_names_are_stable_strings() {
        assert_eq!(ContentType::SearchResult.as_str(), "search-result");
        assert_eq!(ContentType::Json.to_string(), "json");
    }
}
