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
/// Deserializing through `serde_json::Value` is what buys the never-error
/// behavior: any self-describing shape is accepted and then narrowed.
pub fn lenient_reasoning_delta<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::String(text) => {
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        // `null`, object, array, bool and number shapes all carry no text we
        // can use, so they are dropped rather than rejected.
        _ => None,
    })
}
