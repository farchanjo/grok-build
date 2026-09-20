use serde::{Deserialize, Deserializer};

pub fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.filter(|s| !s.is_empty()))
}

/// Deserialize `Option<Option<T>>`: absent (`None`) leaves, `null` (`Some(None)`)
/// clears, a value sets. Requires `#[serde(default, deserialize_with = "…")]`.
pub fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

/// Lenient deserializer for a streaming reasoning/thinking delta.
///
/// Providers disagree on both the wire key and the value shape for reasoning
/// text. A delta is normally a plain string, but real endpoints also send
/// `null` or a structured shape (reasoning-parser metadata objects, detail
/// block arrays, feature flags, token counters). None of those may fail the
/// surrounding chunk, so this helper never returns `Err`:
///
/// * `String` → `Some(s)`; an empty string → `None` (no text to emit).
/// * `null` → `None`.
/// * object / array / bool / number → ignored → `None`.
///
/// Key-absent is handled by `#[serde(default, …)]` — the helper is not called
/// at all when the key is missing, so `default` is mandatory at the call site.
///
/// A visitor over `deserialize_any` replaces the earlier `serde_json::Value`
/// round trip: the never-error behavior is the same, but no intermediate
/// `Value` is built, and the structured shapes (objects, arrays) no longer
/// allocate a map or vec only to be dropped. Reasoning is the longest delta
/// channel, so this runs on the hot path.
pub fn lenient_reasoning_delta<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct ReasoningDeltaVisitor;

    impl<'de> serde::de::Visitor<'de> for ReasoningDeltaVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a reasoning delta string, or any shape to ignore")
        }

        fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
            Ok(non_empty(text.to_owned()))
        }

        fn visit_string<E: serde::de::Error>(self, text: String) -> Result<Self::Value, E> {
            Ok(non_empty(text))
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            // Draining matters: a visitor that returns without consuming leaves
            // the outer deserializer mid-element ("trailing characters",
            // "invalid length ... in map").
            while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
            Ok(None)
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            while map
                .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
                .is_some()
            {}
            Ok(None)
        }
    }

    fn non_empty(text: String) -> Option<String> {
        if text.is_empty() { None } else { Some(text) }
    }

    deserializer.deserialize_any(ReasoningDeltaVisitor)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Probe {
        #[serde(default, deserialize_with = "super::lenient_reasoning_delta")]
        reasoning: Option<String>,
    }

    fn probe(json: &str) -> Option<String> {
        serde_json::from_str::<Probe>(json)
            .expect("no reasoning shape may fail the surrounding chunk")
            .reasoning
    }

    #[test]
    fn keeps_text_and_ignores_every_other_shape() {
        assert_eq!(
            probe(r#"{"reasoning":"We need"}"#).as_deref(),
            Some("We need")
        );
        assert_eq!(probe(r#"{"reasoning":""}"#), None);
        assert_eq!(probe(r#"{"reasoning":null}"#), None);
        assert_eq!(probe(r#"{"reasoning":true}"#), None);
        assert_eq!(probe(r#"{"reasoning":7}"#), None);
        assert_eq!(probe(r#"{"reasoning":1.5}"#), None);
        assert_eq!(probe(r#"{"reasoning":["a","b"]}"#), None);
        assert_eq!(probe(r#"{"reasoning":{"type":"reasoning"}}"#), None);
        // Key absent is `default`; the helper is not called at all.
        assert_eq!(probe("{}"), None);
    }

    #[test]
    fn a_dropped_shape_does_not_consume_the_rest_of_the_chunk() {
        #[derive(Deserialize)]
        struct Both {
            #[serde(default, deserialize_with = "super::lenient_reasoning_delta")]
            reasoning: Option<String>,
            #[serde(default)]
            content: Option<String>,
        }

        let json = r#"{"reasoning":{"a":[1,2,{"b":null}]},"content":"hi"}"#;
        let both: Both = serde_json::from_str(json).unwrap();
        assert_eq!(both.reasoning, None);
        assert_eq!(both.content.as_deref(), Some("hi"));
    }
}
