//! Incremental JSON-string projector for dual-language structured output.
//!
//! When [`ConversationRequest::project_response_field`] is set, the already-
//! normalized L2 stream is wrapped so live [`InferenceChannel::Text`] tokens
//! carry unescaped `response` contents instead of the envelope JSON.
//! Reasoning, tool-call deltas, and [`InferenceEvent::Completed`] assistant
//! text stay raw for terminal schema validation.

use futures_util::Stream;
use futures_util::StreamExt;

use crate::events::{InferenceChannel, InferenceEvent};

/// Wrap `inner` with the incremental `response`-field projector when `enabled`.
/// Disabled is a zero-regression identity path.
pub fn project_response_field<'a, S>(
    inner: S,
    enabled: bool,
) -> impl Stream<Item = InferenceEvent> + Send + 'a
where
    S: Stream<Item = InferenceEvent> + Send + 'a,
{
    let mut projector = ResponseFieldProjector::new(enabled);
    inner.flat_map(move |event| futures_util::stream::iter(projector.process(event)))
}

/// Pure incremental projector state machine.
#[derive(Debug, Clone)]
pub struct ResponseFieldProjector {
    enabled: bool,
    raw: String,
    scan: ScanState,
    unescape: UnescapeState,
    first_token_held: bool,
    first_token_emitted: bool,
}

#[derive(Debug, Clone)]
enum ScanState {
    ExpectObjectStart,
    ExpectKeyOrEnd,
    InKey { buf: String },
    AfterKey { key: String },
    AfterColon { key: String },
    SkipValue { skip: SkipState },
    InResponse,
    AfterValue,
    Done,
}

#[derive(Debug, Clone)]
enum SkipState {
    Decide,
    String,
    Number,
    Literal {
        expected: &'static str,
        pos: usize,
    },
    Nested {
        depth: usize,
        in_string: bool,
        escaped: bool,
    },
}

#[derive(Debug, Clone)]
enum UnescapeState {
    Normal,
    Escape,
    Unicode {
        digits: [u8; 4],
        filled: u8,
    },
    /// High surrogate decoded; next input must be `\uXXXX` of a low surrogate.
    AfterHigh {
        high: u16,
        phase: AfterHighPhase,
    },
}

#[derive(Debug, Clone)]
enum AfterHighPhase {
    ExpectBackslash,
    ExpectU,
    Unicode { digits: [u8; 4], filled: u8 },
}

impl Default for UnescapeState {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug)]
enum SkipStep {
    Continue,
    Complete,
    CompleteAndReprocess,
}

#[derive(Debug)]
enum StringDecode {
    Char(char),
    End,
    Hold,
}

impl ResponseFieldProjector {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            raw: String::new(),
            scan: ScanState::ExpectObjectStart,
            unescape: UnescapeState::Normal,
            first_token_held: false,
            first_token_emitted: false,
        }
    }

    pub fn reset(&mut self) {
        if self.enabled {
            *self = Self::new(true);
        }
    }

    /// Validate the accumulated raw JSON via serde_json.
    pub fn finish(&self) -> Result<serde_json::Value, String> {
        serde_json::from_str(self.raw.trim()).map_err(|e| e.to_string())
    }

    /// Accumulated raw envelope JSON (unprojected).
    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn process(&mut self, event: InferenceEvent) -> Vec<InferenceEvent> {
        if !self.enabled {
            return vec![event];
        }
        match event {
            InferenceEvent::StreamStarted { .. } => {
                self.reset();
                vec![event]
            }
            InferenceEvent::Retrying { .. } => {
                self.reset();
                vec![event]
            }
            InferenceEvent::FirstToken { .. } => {
                self.first_token_held = true;
                Vec::new()
            }
            InferenceEvent::ChannelToken {
                request_id,
                channel: InferenceChannel::Text,
                text,
                chunk_index,
            } => {
                let projected = self.feed_text(&text);
                if projected.is_empty() {
                    return Vec::new();
                }
                let mut out = Vec::with_capacity(2);
                if !self.first_token_emitted {
                    out.push(InferenceEvent::FirstToken {
                        request_id: request_id.clone(),
                    });
                    self.first_token_emitted = true;
                    self.first_token_held = false;
                }
                out.push(InferenceEvent::ChannelToken {
                    request_id,
                    channel: InferenceChannel::Text,
                    text: projected,
                    chunk_index,
                });
                out
            }
            other => vec![other],
        }
    }

    /// Feed a text delta. Returns newly decoded `response` contents.
    pub fn feed_text(&mut self, delta: &str) -> String {
        if !self.enabled {
            return delta.to_owned();
        }
        self.raw.push_str(delta);
        let mut emitted = String::new();
        for ch in delta.chars() {
            self.feed_char(ch, &mut emitted);
        }
        emitted
    }

    fn feed_char(&mut self, ch: char, emitted: &mut String) {
        let mut current = Some(ch);
        while let Some(c) = current.take() {
            match &mut self.scan {
                ScanState::Done => return,
                ScanState::ExpectObjectStart => {
                    if c.is_ascii_whitespace() {
                        return;
                    }
                    if c == '{' {
                        self.scan = ScanState::ExpectKeyOrEnd;
                    }
                }
                ScanState::ExpectKeyOrEnd => {
                    if c.is_ascii_whitespace() {
                        return;
                    }
                    if c == '}' {
                        self.scan = ScanState::Done;
                    } else if c == '"' {
                        self.unescape = UnescapeState::Normal;
                        self.scan = ScanState::InKey { buf: String::new() };
                    }
                }
                ScanState::InKey { buf } => match decode_string_char(&mut self.unescape, c) {
                    StringDecode::Char(decoded) => buf.push(decoded),
                    StringDecode::End => {
                        let key = std::mem::take(buf);
                        self.unescape = UnescapeState::Normal;
                        self.scan = ScanState::AfterKey { key };
                    }
                    StringDecode::Hold => {}
                },
                ScanState::AfterKey { key } => {
                    if c.is_ascii_whitespace() {
                        return;
                    }
                    if c == ':' {
                        let key = std::mem::take(key);
                        self.scan = ScanState::AfterColon { key };
                    }
                }
                ScanState::AfterColon { key } => {
                    if c.is_ascii_whitespace() {
                        return;
                    }
                    if key == "response" && c == '"' {
                        self.unescape = UnescapeState::Normal;
                        self.scan = ScanState::InResponse;
                    } else {
                        let _ = std::mem::take(key);
                        self.unescape = UnescapeState::Normal;
                        self.scan = ScanState::SkipValue {
                            skip: SkipState::Decide,
                        };
                        current = Some(c);
                    }
                }
                ScanState::SkipValue { skip } => match skip_value_char(skip, &mut self.unescape, c)
                {
                    SkipStep::Continue => {}
                    SkipStep::Complete => {
                        self.unescape = UnescapeState::Normal;
                        self.scan = ScanState::AfterValue;
                    }
                    SkipStep::CompleteAndReprocess => {
                        self.unescape = UnescapeState::Normal;
                        self.scan = ScanState::AfterValue;
                        current = Some(c);
                    }
                },
                ScanState::InResponse => match decode_string_char(&mut self.unescape, c) {
                    StringDecode::Char(decoded) => emitted.push(decoded),
                    StringDecode::End => {
                        self.unescape = UnescapeState::Normal;
                        self.scan = ScanState::AfterValue;
                    }
                    StringDecode::Hold => {}
                },
                ScanState::AfterValue => {
                    if c.is_ascii_whitespace() {
                        return;
                    }
                    if c == ',' {
                        self.scan = ScanState::ExpectKeyOrEnd;
                    } else if c == '}' {
                        self.scan = ScanState::Done;
                    }
                }
            }
        }
    }
}

fn hex_digit(ch: char) -> Option<u8> {
    ch.to_digit(16).map(|d| d as u8)
}

fn decode_hex4(digits: &[u8; 4]) -> u16 {
    digits
        .iter()
        .fold(0u16, |acc, d| (acc << 4) | u16::from(*d))
}

fn combine_surrogates(high: u16, low: u16) -> char {
    let cp = 0x10000 + ((u32::from(high) - 0xD800) << 10) + (u32::from(low) - 0xDC00);
    char::from_u32(cp).unwrap_or('\u{FFFD}')
}

fn decode_string_char(state: &mut UnescapeState, ch: char) -> StringDecode {
    match state {
        UnescapeState::Normal => match ch {
            '"' => StringDecode::End,
            '\\' => {
                *state = UnescapeState::Escape;
                StringDecode::Hold
            }
            other => StringDecode::Char(other),
        },
        UnescapeState::Escape => match ch {
            '"' => {
                *state = UnescapeState::Normal;
                StringDecode::Char('"')
            }
            '\\' => {
                *state = UnescapeState::Normal;
                StringDecode::Char('\\')
            }
            '/' => {
                *state = UnescapeState::Normal;
                StringDecode::Char('/')
            }
            'b' => {
                *state = UnescapeState::Normal;
                StringDecode::Char('\u{0008}')
            }
            'f' => {
                *state = UnescapeState::Normal;
                StringDecode::Char('\u{000c}')
            }
            'n' => {
                *state = UnescapeState::Normal;
                StringDecode::Char('\n')
            }
            'r' => {
                *state = UnescapeState::Normal;
                StringDecode::Char('\r')
            }
            't' => {
                *state = UnescapeState::Normal;
                StringDecode::Char('\t')
            }
            'u' => {
                *state = UnescapeState::Unicode {
                    digits: [0; 4],
                    filled: 0,
                };
                StringDecode::Hold
            }
            other => {
                *state = UnescapeState::Normal;
                StringDecode::Char(other)
            }
        },
        UnescapeState::Unicode { digits, filled } => {
            let Some(digit) = hex_digit(ch) else {
                *state = UnescapeState::Normal;
                return StringDecode::Char(ch);
            };
            digits[*filled as usize] = digit;
            *filled += 1;
            if *filled < 4 {
                return StringDecode::Hold;
            }
            let unit = decode_hex4(digits);
            if (0xD800..=0xDBFF).contains(&unit) {
                *state = UnescapeState::AfterHigh {
                    high: unit,
                    phase: AfterHighPhase::ExpectBackslash,
                };
                return StringDecode::Hold;
            }
            if (0xDC00..=0xDFFF).contains(&unit) {
                *state = UnescapeState::Normal;
                return StringDecode::Char('\u{FFFD}');
            }
            *state = UnescapeState::Normal;
            StringDecode::Char(char::from_u32(u32::from(unit)).unwrap_or('\u{FFFD}'))
        }
        UnescapeState::AfterHigh { high, phase } => match phase {
            AfterHighPhase::ExpectBackslash => {
                if ch == '\\' {
                    *phase = AfterHighPhase::ExpectU;
                    StringDecode::Hold
                } else {
                    *state = UnescapeState::Normal;
                    StringDecode::Char('\u{FFFD}')
                }
            }
            AfterHighPhase::ExpectU => {
                if ch == 'u' {
                    *phase = AfterHighPhase::Unicode {
                        digits: [0; 4],
                        filled: 0,
                    };
                    StringDecode::Hold
                } else {
                    *state = UnescapeState::Normal;
                    StringDecode::Char('\u{FFFD}')
                }
            }
            AfterHighPhase::Unicode { digits, filled } => {
                let Some(digit) = hex_digit(ch) else {
                    *state = UnescapeState::Normal;
                    return StringDecode::Char('\u{FFFD}');
                };
                digits[*filled as usize] = digit;
                *filled += 1;
                if *filled < 4 {
                    return StringDecode::Hold;
                }
                let low = decode_hex4(digits);
                let high = *high;
                *state = UnescapeState::Normal;
                if (0xDC00..=0xDFFF).contains(&low) {
                    StringDecode::Char(combine_surrogates(high, low))
                } else {
                    StringDecode::Char('\u{FFFD}')
                }
            }
        },
    }
}

fn skip_value_char(skip: &mut SkipState, unescape: &mut UnescapeState, ch: char) -> SkipStep {
    match skip {
        SkipState::Decide => {
            if ch.is_ascii_whitespace() {
                return SkipStep::Continue;
            }
            match ch {
                '"' => {
                    *unescape = UnescapeState::Normal;
                    *skip = SkipState::String;
                    SkipStep::Continue
                }
                '{' | '[' => {
                    *skip = SkipState::Nested {
                        depth: 1,
                        in_string: false,
                        escaped: false,
                    };
                    SkipStep::Continue
                }
                't' => {
                    *skip = SkipState::Literal {
                        expected: "true",
                        pos: 1,
                    };
                    SkipStep::Continue
                }
                'f' => {
                    *skip = SkipState::Literal {
                        expected: "false",
                        pos: 1,
                    };
                    SkipStep::Continue
                }
                'n' => {
                    *skip = SkipState::Literal {
                        expected: "null",
                        pos: 1,
                    };
                    SkipStep::Continue
                }
                '-' | '0'..='9' => {
                    *skip = SkipState::Number;
                    SkipStep::Continue
                }
                _ => SkipStep::CompleteAndReprocess,
            }
        }
        SkipState::String => match decode_string_char(unescape, ch) {
            StringDecode::End => SkipStep::Complete,
            _ => SkipStep::Continue,
        },
        SkipState::Number => {
            if matches!(ch, '0'..='9' | '.' | 'e' | 'E' | '+' | '-') {
                SkipStep::Continue
            } else {
                SkipStep::CompleteAndReprocess
            }
        }
        SkipState::Literal { expected, pos } => {
            if *pos < expected.len() && expected.as_bytes()[*pos] == ch as u8 {
                *pos += 1;
                if *pos >= expected.len() {
                    SkipStep::Complete
                } else {
                    SkipStep::Continue
                }
            } else {
                SkipStep::CompleteAndReprocess
            }
        }
        SkipState::Nested {
            depth,
            in_string,
            escaped,
        } => {
            if *in_string {
                if *escaped {
                    *escaped = false;
                } else if ch == '\\' {
                    *escaped = true;
                } else if ch == '"' {
                    *in_string = false;
                }
                return SkipStep::Continue;
            }
            match ch {
                '"' => *in_string = true,
                '{' | '[' => *depth += 1,
                '}' | ']' => {
                    *depth = depth.saturating_sub(1);
                    if *depth == 0 {
                        return SkipStep::Complete;
                    }
                }
                _ => {}
            }
            SkipStep::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{InferenceChannel, InferenceEvent};
    use crate::metrics::InferenceLatencyStats;
    use crate::types::RequestId;
    use xai_grok_inference_types::{ConversationItem, ConversationResponse};

    fn rid() -> RequestId {
        RequestId::from("proj-test")
    }

    fn text_token(text: &str, idx: u64) -> InferenceEvent {
        InferenceEvent::ChannelToken {
            request_id: rid(),
            channel: InferenceChannel::Text,
            text: text.to_owned(),
            chunk_index: idx,
        }
    }

    fn projected_text(events: &[InferenceEvent]) -> String {
        events
            .iter()
            .filter_map(|e| match e {
                InferenceEvent::ChannelToken {
                    channel: InferenceChannel::Text,
                    text,
                    ..
                } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn first_token_count(events: &[InferenceEvent]) -> usize {
        events
            .iter()
            .filter(|e| matches!(e, InferenceEvent::FirstToken { .. }))
            .count()
    }

    fn envelope(response: &str, conv_first: bool) -> String {
        let escaped = serde_json::to_string(response).expect("string");
        if conv_first {
            format!(
                r#"{{"conversation_language":"pt-BR","artifact_language":"en-US","response":{escaped}}}"#
            )
        } else {
            format!(
                r#"{{"response":{escaped},"conversation_language":"pt-BR","artifact_language":"en-US"}}"#
            )
        }
    }

    fn project_all(json: &str) -> (String, Result<serde_json::Value, String>) {
        let mut p = ResponseFieldProjector::new(true);
        let out = p.feed_text(json);
        (out, p.finish())
    }

    fn project_chunks<'a>(chunks: impl IntoIterator<Item = &'a str>) -> String {
        let mut p = ResponseFieldProjector::new(true);
        let mut out = String::new();
        for c in chunks {
            out.push_str(&p.feed_text(c));
        }
        out
    }

    fn empty_response(items: Vec<ConversationItem>) -> ConversationResponse {
        ConversationResponse {
            items,
            stop_reason: None,
            usage: None,
            cost_usd_ticks: None,
            message_chunks_emitted: 0,
            doom_loop_signals: Vec::new(),
            stop_message: None,
            fallback_served_model: None,
        }
    }

    #[test]
    fn key_order_languages_then_response() {
        let json = envelope("olá mundo", true);
        let (out, parsed) = project_all(&json);
        assert_eq!(out, "olá mundo");
        let v = parsed.expect("valid json");
        assert_eq!(v["response"], "olá mundo");
        assert_eq!(v["conversation_language"], "pt-BR");
    }

    #[test]
    fn key_order_response_first() {
        let json = envelope("hello", false);
        let (out, parsed) = project_all(&json);
        assert_eq!(out, "hello");
        parsed.expect("valid json");
    }

    #[test]
    fn every_json_string_escape() {
        let raw = "quote:\" slash:\\ solidus:/ bs:\u{0008} ff:\u{000c} nl:\n cr:\r tab:\t";
        let json = envelope(raw, true);
        let (out, parsed) = project_all(&json);
        assert_eq!(out, raw);
        parsed.expect("valid json");
    }

    #[test]
    fn unicode_escape_basic_multilingual() {
        let json = r#"{"response":"caf\u00e9","conversation_language":"fr-FR","artifact_language":"en-US"}"#;
        let (out, parsed) = project_all(json);
        assert_eq!(out, "café");
        parsed.expect("valid json");
    }

    #[test]
    fn surrogate_pair_emoji() {
        let json = r#"{"response":"\uD83D\uDE00","conversation_language":"en-US","artifact_language":"en-US"}"#;
        let (out, parsed) = project_all(json);
        assert_eq!(out, "😀");
        parsed.expect("valid json");
    }

    #[test]
    fn surrogate_pair_split_at_every_boundary() {
        let json = r#"{"response":"\uD83D\uDE00"}"#;
        for split in 1..json.len() {
            let mut p = ResponseFieldProjector::new(true);
            let a = &json[..split];
            let b = &json[split..];
            let mut out = String::new();
            out.push_str(&p.feed_text(a));
            out.push_str(&p.feed_text(b));
            assert_eq!(out, "😀", "split at {split}: {a:?} + {b:?}");
        }
    }

    #[test]
    fn chunked_at_every_byte_boundary() {
        let json = envelope("hello-world", false);
        let mut p = ResponseFieldProjector::new(true);
        let mut out = String::new();
        for i in 0..json.len() {
            out.push_str(&p.feed_text(&json[i..i + 1]));
        }
        assert_eq!(out, "hello-world");
        p.finish().expect("valid json");
    }

    #[test]
    fn disabled_identity() {
        let mut p = ResponseFieldProjector::new(false);
        let event = text_token(r#"{"response":"nope"}"#, 0);
        let out = p.process(event);
        assert_eq!(out.len(), 1);
        match &out[0] {
            InferenceEvent::ChannelToken { text, .. } => {
                assert_eq!(text, r#"{"response":"nope"}"#);
            }
            other => panic!("unexpected {other:?}"),
        }
        let first = InferenceEvent::FirstToken { request_id: rid() };
        let out = p.process(first);
        assert!(matches!(out[0], InferenceEvent::FirstToken { .. }));
    }

    #[test]
    fn invalid_json_finish_errors() {
        let mut p = ResponseFieldProjector::new(true);
        let _ = p.feed_text("{not json");
        assert!(p.finish().is_err());
    }

    #[test]
    fn trailing_garbage_finish_errors() {
        let json = format!("{} trailing", envelope("ok", true));
        let mut p = ResponseFieldProjector::new(true);
        let out = p.feed_text(&json);
        assert_eq!(out, "ok");
        assert!(
            p.finish().is_err(),
            "trailing garbage must fail terminal parse"
        );
    }

    #[test]
    fn delays_first_token_until_projected_char() {
        let mut p = ResponseFieldProjector::new(true);
        let mut events = Vec::new();
        events.extend(p.process(InferenceEvent::StreamStarted {
            request_id: rid(),
            timestamp_ms: 0,
        }));
        events.extend(p.process(InferenceEvent::FirstToken { request_id: rid() }));
        assert_eq!(
            first_token_count(&events),
            0,
            "FirstToken held until response"
        );

        let prefix = r#"{"conversation_language":"pt-BR","artifact_language":"en-US","response":""#;
        events.extend(p.process(text_token(prefix, 0)));
        assert_eq!(first_token_count(&events), 0);
        assert!(projected_text(&events).is_empty());

        events.extend(p.process(text_token("Hi", 1)));
        assert_eq!(first_token_count(&events), 1);
        assert_eq!(projected_text(&events), "Hi");
    }

    #[test]
    fn reasoning_and_tool_deltas_pass_through() {
        let mut p = ResponseFieldProjector::new(true);
        let reasoning = InferenceEvent::ChannelToken {
            request_id: rid(),
            channel: InferenceChannel::Reasoning,
            text: "think".into(),
            chunk_index: 0,
        };
        let out = p.process(reasoning);
        match &out[0] {
            InferenceEvent::ChannelToken {
                channel: InferenceChannel::Reasoning,
                text,
                ..
            } => assert_eq!(text, "think"),
            other => panic!("{other:?}"),
        }
        let tool = InferenceEvent::ToolCallDelta {
            request_id: rid(),
            tool_index: 0,
            id: Some("c1".into()),
            name: Some("read_file".into()),
            arguments_delta: Some("{".into()),
        };
        let out = p.process(tool);
        assert!(matches!(out[0], InferenceEvent::ToolCallDelta { .. }));
    }

    #[test]
    fn completed_keeps_raw_assistant_text() {
        let mut p = ResponseFieldProjector::new(true);
        let raw = envelope("visible", true);
        let _ = p.feed_text(&raw);
        let completed = InferenceEvent::Completed {
            request_id: rid(),
            response: Box::new(empty_response(vec![ConversationItem::assistant(
                raw.clone(),
            )])),
            metrics: InferenceLatencyStats::default(),
        };
        let out = p.process(completed);
        match &out[0] {
            InferenceEvent::Completed { response, .. } => {
                assert_eq!(response.assistant_text(), raw);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn stream_started_resets_state() {
        let mut p = ResponseFieldProjector::new(true);
        let _ = p.feed_text(r#"{"response":"one""#);
        let _ = p.process(InferenceEvent::StreamStarted {
            request_id: rid(),
            timestamp_ms: 1,
        });
        assert!(p.raw().is_empty());
        let out = p.feed_text(&envelope("two", false));
        assert_eq!(out, "two");
    }

    #[test]
    fn skips_non_response_values_generically() {
        let json = r#"{"conversation_language":"ja-JP","nested":{"a":[1,true,null,"x"]},"artifact_language":"en-US","response":"ok"}"#;
        let (out, parsed) = project_all(json);
        assert_eq!(out, "ok");
        parsed.expect("valid json");
    }

    #[test]
    fn holds_incomplete_unicode_escape_across_chunks() {
        let mut p = ResponseFieldProjector::new(true);
        let a = r#"{"response":"\u00"#;
        let b = r#"e9"}"#;
        assert!(p.feed_text(a).is_empty());
        assert_eq!(p.feed_text(b), "é");
    }

    #[test]
    fn holds_incomplete_backslash_across_chunks() {
        let mut p = ResponseFieldProjector::new(true);
        let mut out = String::new();
        out.push_str(&p.feed_text(r#"{"response":"a\"#));
        out.push_str(&p.feed_text(r#"n"}"#));
        assert_eq!(out, "a\n");
    }

    #[tokio::test]
    async fn project_response_field_stream_identity_when_disabled() {
        let events = vec![
            InferenceEvent::FirstToken { request_id: rid() },
            text_token("raw", 0),
        ];
        let collected: Vec<_> = project_response_field(futures_util::stream::iter(events), false)
            .collect()
            .await;
        assert_eq!(collected.len(), 2);
        assert!(matches!(collected[0], InferenceEvent::FirstToken { .. }));
        match &collected[1] {
            InferenceEvent::ChannelToken { text, .. } => assert_eq!(text, "raw"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn whitespace_and_pretty_json() {
        let json = r#"{
            "artifact_language": "en-US",
            "conversation_language": "pt-BR",
            "response": "ok"
        }"#;
        let (out, parsed) = project_all(json);
        assert_eq!(out, "ok");
        parsed.expect("valid json");
    }

    #[test]
    fn empty_response_emits_nothing() {
        let json = envelope("", true);
        let (out, parsed) = project_all(&json);
        assert!(out.is_empty());
        parsed.expect("valid json");
    }

    #[test]
    fn project_chunks_response_first_split_on_key() {
        let json = envelope("abc", false);
        let mid = json.find("abc").expect("response body");
        assert_eq!(project_chunks([&json[..mid], &json[mid..]]), "abc");
    }
}
