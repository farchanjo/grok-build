//! Minimal standard base64 → float32 LE decoder for embedding wire payloads.
//!
//! Handwritten to avoid a new crate dependency. Accepts standard base64 with
//! optional padding; rejects URL-safe alphabet variants and non-alphabet noise.

use super::types::{MAX_EMBEDDING_DIMENSIONS, RetrievalError, RetrievalResult};

const TABLE: [i8; 256] = {
    let mut t = [-1i8; 256];
    let mut i = 0u8;
    while i < 26 {
        t[(b'A' + i) as usize] = i as i8;
        t[(b'a' + i) as usize] = (26 + i) as i8;
        i += 1;
    }
    i = 0;
    while i < 10 {
        t[(b'0' + i) as usize] = (52 + i) as i8;
        i += 1;
    }
    t[b'+' as usize] = 62;
    t[b'/' as usize] = 63;
    t
};

/// Decode standard base64 into little-endian f32 components.
pub fn decode_base64_f32(encoded: &str) -> RetrievalResult<Vec<f32>> {
    let bytes = decode_standard_base64(encoded.trim())?;
    if bytes.is_empty() {
        return Err(RetrievalError::MalformedResponse(
            "base64 embedding payload is empty".into(),
        ));
    }
    if bytes.len() % 4 != 0 {
        return Err(RetrievalError::MalformedResponse(format!(
            "base64 embedding byte length {} is not a multiple of 4",
            bytes.len()
        )));
    }
    let n = bytes.len() / 4;
    if n > MAX_EMBEDDING_DIMENSIONS {
        return Err(RetrievalError::MalformedResponse(format!(
            "base64 embedding dimensions {n} exceed max {MAX_EMBEDDING_DIMENSIONS}"
        )));
    }
    let mut out = Vec::with_capacity(n);
    for chunk in bytes.chunks_exact(4) {
        let arr = [chunk[0], chunk[1], chunk[2], chunk[3]];
        let f = f32::from_le_bytes(arr);
        if !f.is_finite() {
            return Err(RetrievalError::MalformedResponse(
                "base64 embedding contains non-finite float32".into(),
            ));
        }
        out.push(f);
    }
    Ok(out)
}

fn decode_standard_base64(input: &str) -> RetrievalResult<Vec<u8>> {
    let raw = input.as_bytes();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    // Strip whitespace.
    let mut filtered = Vec::with_capacity(raw.len());
    for &b in raw {
        if b == b' ' || b == b'\n' || b == b'\r' || b == b'\t' {
            continue;
        }
        filtered.push(b);
    }
    if filtered.is_empty() {
        return Ok(Vec::new());
    }
    if filtered.len() % 4 != 0 {
        return Err(RetrievalError::MalformedResponse(
            "corrupt base64 embedding: length not multiple of 4".into(),
        ));
    }
    let mut out = Vec::with_capacity(filtered.len() / 4 * 3);
    let mut i = 0;
    while i < filtered.len() {
        let b0 = filtered[i];
        let b1 = filtered[i + 1];
        let b2 = filtered[i + 2];
        let b3 = filtered[i + 3];
        i += 4;

        let pad2 = b2 == b'=';
        let pad3 = b3 == b'=';
        if pad2 && !pad3 {
            return Err(RetrievalError::MalformedResponse(
                "corrupt base64 embedding: invalid padding".into(),
            ));
        }
        if (pad2 || pad3) && i != filtered.len() {
            return Err(RetrievalError::MalformedResponse(
                "corrupt base64 embedding: padding not at end".into(),
            ));
        }

        let v0 = decode_char(b0)?;
        let v1 = decode_char(b1)?;
        let v2 = if pad2 { 0 } else { decode_char(b2)? };
        let v3 = if pad3 { 0 } else { decode_char(b3)? };

        out.push(((v0 << 2) | (v1 >> 4)) as u8);
        if !pad2 {
            out.push(((v1 << 4) | (v2 >> 2)) as u8);
        }
        if !pad3 {
            out.push(((v2 << 6) | v3) as u8);
        }
    }
    Ok(out)
}

fn decode_char(b: u8) -> RetrievalResult<u8> {
    let v = TABLE[b as usize];
    if v < 0 {
        return Err(RetrievalError::MalformedResponse(
            "corrupt base64 embedding: invalid character".into(),
        ));
    }
    Ok(v as u8)
}

/// Encode bytes as standard base64 (tests / fixtures only).
#[cfg(test)]
pub fn encode_standard_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | (bytes[i + 2] as u32);
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 6) & 63) as usize] as char);
        out.push(ALPHABET[(n & 63) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let n = (bytes[i] as u32) << 16;
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8);
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_f32() {
        let floats = [1.0f32, -2.5, 0.0, 3.14159];
        let mut bytes = Vec::new();
        for f in floats {
            bytes.extend_from_slice(&f.to_le_bytes());
        }
        let b64 = encode_standard_base64(&bytes);
        let decoded = decode_base64_f32(&b64).unwrap();
        assert_eq!(decoded, floats);
    }

    #[test]
    fn rejects_bad_length_and_alphabet() {
        assert!(decode_base64_f32(&encode_standard_base64(&[1, 2, 3])).is_err());
        assert!(decode_base64_f32("!!!").is_err());
    }
}
