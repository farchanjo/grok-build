//! OpenAI-compatible embeddings adapter.
//!
//! Usable by built-in OpenAI/OpenRouter where compatible, configured same-kind
//! accounts, and custom OpenAI-compatible / retrieval-only providers declaring
//! embeddings. Handwritten request/response types support float and base64
//! encodings (generated OpenAI `Embedding.embedding` is `Vec<f64>` only).

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::base64_f32::decode_base64_f32;
use super::transport::{RetrievalCredential, RetrievalTransport};
use super::types::{
    EmbeddingAdapter, EmbeddingEncodingFormat, EmbeddingRequest, EmbeddingResult, EmbeddingVector,
    MAX_EMBEDDING_DIMENSIONS, RetrievalError, RetrievalResult, RetrievalRouteContext,
    normalize_endpoint_path, validate_embedding_request,
};

/// Typed OpenAI-compatible embeddings client bound to one exact route.
#[derive(Debug, Clone)]
pub struct OpenaiCompatibleEmbeddings {
    route: RetrievalRouteContext,
    transport: RetrievalTransport,
}

impl OpenaiCompatibleEmbeddings {
    pub fn new(route: RetrievalRouteContext) -> RetrievalResult<Self> {
        let transport = RetrievalTransport::from_route(&route)?;
        Ok(Self { route, transport })
    }

    pub async fn embed(
        &self,
        request: EmbeddingRequest,
        credential: &RetrievalCredential,
        cancel: CancellationToken,
    ) -> RetrievalResult<EmbeddingResult> {
        validate_embedding_request(&request)?;
        let endpoint = normalize_endpoint_path(&request.endpoint);
        let mut body = json!({
            "model": request.model,
            "input": if request.inputs.len() == 1 {
                Value::String(request.inputs[0].clone())
            } else {
                Value::Array(
                    request
                        .inputs
                        .iter()
                        .map(|s| Value::String(s.clone()))
                        .collect(),
                )
            },
            "encoding_format": request.encoding.as_str(),
        });
        if let Some(dims) = request.dimensions {
            body.as_object_mut()
                .expect("object")
                .insert("dimensions".into(), json!(dims));
        }

        let raw = self
            .transport
            .post_json(&endpoint, &body, credential, &cancel, "createEmbedding")
            .await?;
        parse_embedding_response(
            &raw,
            request.inputs.len(),
            request.dimensions,
            request.encoding,
            &request.model,
        )
    }
}

impl EmbeddingAdapter for OpenaiCompatibleEmbeddings {
    async fn embed(
        &self,
        request: EmbeddingRequest,
        credential: &RetrievalCredential,
        cancel: CancellationToken,
    ) -> RetrievalResult<EmbeddingResult> {
        OpenaiCompatibleEmbeddings::embed(self, request, credential, cancel).await
    }

    fn route_context(&self) -> &RetrievalRouteContext {
        &self.route
    }
}

/// Wire item supporting float arrays or base64-encoded float32 payloads.
#[derive(Debug, Deserialize)]
struct WireEmbeddingItem {
    index: i64,
    embedding: WireEmbeddingPayload,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WireEmbeddingPayload {
    Float(Vec<f64>),
    Base64(String),
}

#[derive(Debug, Deserialize)]
struct WireEmbeddingResponse {
    #[serde(default)]
    data: Vec<WireEmbeddingItem>,
    #[serde(default)]
    model: Option<String>,
}

fn parse_embedding_response(
    raw: &Value,
    expected_count: usize,
    expected_dims: Option<u32>,
    encoding: EmbeddingEncodingFormat,
    fallback_model: &str,
) -> RetrievalResult<EmbeddingResult> {
    let parsed: WireEmbeddingResponse = serde_json::from_value(raw.clone())
        .map_err(|e| RetrievalError::MalformedResponse(format!("embedding envelope: {e}")))?;

    if parsed.data.is_empty() {
        return Err(RetrievalError::MalformedResponse(
            "embedding response data is empty".into(),
        ));
    }
    if parsed.data.len() != expected_count {
        return Err(RetrievalError::MalformedResponse(format!(
            "embedding result count {} != expected {expected_count}",
            parsed.data.len()
        )));
    }

    let mut seen = HashSet::with_capacity(expected_count);
    let mut slots: Vec<Option<Vec<f32>>> = vec![None; expected_count];

    for item in parsed.data {
        if item.index < 0 || item.index as usize >= expected_count {
            return Err(RetrievalError::MalformedResponse(format!(
                "embedding index {} out of range 0..{expected_count}",
                item.index
            )));
        }
        let idx = item.index as usize;
        if !seen.insert(idx) {
            return Err(RetrievalError::MalformedResponse(format!(
                "duplicate embedding index {idx}"
            )));
        }
        let values = match (&item.embedding, encoding) {
            (WireEmbeddingPayload::Float(v), EmbeddingEncodingFormat::Float)
            | (WireEmbeddingPayload::Float(v), EmbeddingEncodingFormat::Base64) => {
                // Accept float arrays even if base64 was requested (some proxies).
                decode_float_vec(v)?
            }
            (WireEmbeddingPayload::Base64(s), _) => decode_base64_f32(s)?,
        };
        if values.is_empty() {
            return Err(RetrievalError::MalformedResponse(format!(
                "embedding[{idx}] is empty"
            )));
        }
        if values.len() > MAX_EMBEDDING_DIMENSIONS {
            return Err(RetrievalError::MalformedResponse(format!(
                "embedding[{idx}] dimensions {} exceed max {MAX_EMBEDDING_DIMENSIONS}",
                values.len()
            )));
        }
        if let Some(dims) = expected_dims {
            if values.len() != dims as usize {
                return Err(RetrievalError::MalformedResponse(format!(
                    "embedding[{idx}] dimensions {} != configured {dims}",
                    values.len()
                )));
            }
        }
        if values.iter().any(|f| !f.is_finite()) {
            return Err(RetrievalError::MalformedResponse(format!(
                "embedding[{idx}] contains non-finite values"
            )));
        }
        slots[idx] = Some(values);
    }

    // Exact count already checked; ensure no holes (defensive).
    let mut vectors: Vec<EmbeddingVector> = Vec::with_capacity(expected_count);
    for (index, slot) in slots.into_iter().enumerate() {
        let values = slot.ok_or_else(|| {
            RetrievalError::MalformedResponse(format!("missing embedding for index {index}"))
        })?;
        // Ragged dimensions across the batch are rejected when configured dims
        // are absent: all vectors must share the first vector's length.
        if let Some(first) = vectors.first() {
            if first.values.len() != values.len() {
                return Err(RetrievalError::MalformedResponse(format!(
                    "ragged embedding dimensions at index {index}: {} != {}",
                    values.len(),
                    first.values.len()
                )));
            }
        }
        vectors.push(EmbeddingVector { index, values });
    }

    Ok(EmbeddingResult {
        model: parsed
            .model
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| fallback_model.to_owned()),
        vectors,
    })
}

fn decode_float_vec(v: &[f64]) -> RetrievalResult<Vec<f32>> {
    let mut out = Vec::with_capacity(v.len());
    for (i, x) in v.iter().enumerate() {
        if !x.is_finite() {
            return Err(RetrievalError::MalformedResponse(format!(
                "non-finite float at component {i}"
            )));
        }
        out.push(*x as f32);
    }
    Ok(out)
}

/// Pure response parsing entry for unit tests without HTTP.
pub fn parse_embedding_response_for_test(
    raw: &Value,
    expected_count: usize,
    expected_dims: Option<u32>,
    encoding: EmbeddingEncodingFormat,
    fallback_model: &str,
) -> RetrievalResult<EmbeddingResult> {
    parse_embedding_response(raw, expected_count, expected_dims, encoding, fallback_model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::base64_f32::encode_standard_base64;

    #[test]
    fn float_happy_reorders_by_index() {
        let raw = json!({
            "model": "text-embedding-3-small",
            "data": [
                {"index": 1, "embedding": [0.0, 1.0]},
                {"index": 0, "embedding": [1.0, 0.0]}
            ]
        });
        let res =
            parse_embedding_response(&raw, 2, Some(2), EmbeddingEncodingFormat::Float, "fallback")
                .unwrap();
        assert_eq!(res.vectors[0].values, vec![1.0, 0.0]);
        assert_eq!(res.vectors[1].values, vec![0.0, 1.0]);
        assert_eq!(res.model, "text-embedding-3-small");
    }

    #[test]
    fn rejects_duplicate_and_oob_and_count() {
        let dup = json!({"data": [
            {"index": 0, "embedding": [1.0]},
            {"index": 0, "embedding": [2.0]}
        ]});
        assert!(
            parse_embedding_response(&dup, 2, None, EmbeddingEncodingFormat::Float, "m").is_err()
        );
        let oob = json!({"data": [{"index": 5, "embedding": [1.0]}]});
        assert!(
            parse_embedding_response(&oob, 1, None, EmbeddingEncodingFormat::Float, "m").is_err()
        );
        let count = json!({"data": [{"index": 0, "embedding": [1.0]}]});
        assert!(
            parse_embedding_response(&count, 2, None, EmbeddingEncodingFormat::Float, "m").is_err()
        );
    }

    #[test]
    fn rejects_dimension_ragged_nonfinite() {
        let dim = json!({"data": [{"index": 0, "embedding": [1.0, 2.0]}]});
        assert!(
            parse_embedding_response(&dim, 1, Some(3), EmbeddingEncodingFormat::Float, "m")
                .is_err()
        );
        let ragged = json!({"data": [
            {"index": 0, "embedding": [1.0, 2.0]},
            {"index": 1, "embedding": [1.0]}
        ]});
        assert!(
            parse_embedding_response(&ragged, 2, None, EmbeddingEncodingFormat::Float, "m")
                .is_err()
        );
        let nan = json!({"data": [{"index": 0, "embedding": [f64::NAN]}]});
        assert!(
            parse_embedding_response(&nan, 1, None, EmbeddingEncodingFormat::Float, "m").is_err()
        );
    }

    #[test]
    fn base64_float32_le_roundtrip() {
        let floats = [1.0f32, -2.5, 0.0];
        let mut bytes = Vec::new();
        for f in floats {
            bytes.extend_from_slice(&f.to_le_bytes());
        }
        let b64 = encode_standard_base64(&bytes);
        let decoded = decode_base64_f32(&b64).unwrap();
        assert_eq!(decoded, floats);
        assert!(decode_base64_f32(&encode_standard_base64(&[1u8, 2, 3])).is_err());
        assert!(decode_base64_f32("!!!not-b64!!!").is_err());
    }

    #[test]
    fn base64_in_response() {
        let floats = [0.5f32, 1.5];
        let mut bytes = Vec::new();
        for f in floats {
            bytes.extend_from_slice(&f.to_le_bytes());
        }
        let b64 = encode_standard_base64(&bytes);
        let raw = json!({"data": [{"index": 0, "embedding": b64}]});
        let res = parse_embedding_response(&raw, 1, Some(2), EmbeddingEncodingFormat::Base64, "m")
            .unwrap();
        assert_eq!(res.vectors[0].values, floats);
    }
}
