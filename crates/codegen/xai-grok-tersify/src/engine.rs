//! The engine core: detect -> route -> compress -> measure -> store.
//!
//! This module owns everything a compressor must not: token counting, the
//! recovery store, and the decision of whether a result may be emitted at all.
//! The emit rule is the crate's central fail-closed contract —
//!
//! > a lossy result is only emitted when its original is recoverable *and* the
//! > result is smaller in tokens; otherwise the original bytes pass through
//! > unchanged and nothing is claimed.
//!
//! `Mode::Record` is the safe default: the engine runs the pipeline on copies,
//! reports what compression would do, and hands back the original bytes. An
//! unrecognized mode string also resolves to `Record`, never to a live mode.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::compressors::Compressor;
use crate::detect::ContentType;
use crate::safety;
use crate::tokens::{self, Counter, TokenEstimate};

/// Runtime mode. Unknown strings from config resolve to [`Mode::Record`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Run the pipeline, report the estimate, emit the original bytes.
    #[default]
    Record,
    /// Emit compressed bytes when a compressor applies and the result is
    /// smaller; lossy results additionally require a store.
    Compress,
}

impl Mode {
    /// Parse a config value. Anything unrecognized fails closed to `Record` so
    /// a typo can never silently enable transforms.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "compress" => Mode::Compress,
            _ => Mode::Record,
        }
    }
}

/// Per-call options.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Force a content type instead of detecting. Forced types are still
    /// subject to every emit rule.
    pub content_type: Option<ContentType>,
}

/// The outcome of one [`Engine::compress`] call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Result {
    /// The bytes to show the model. Equals the input whenever nothing applied.
    pub body: Vec<u8>,
    /// Content type that was used, after detection or forcing.
    pub content_type: ContentType,
    /// Whether `body` differs from the input (mode Compress, compressor applied,
    /// smaller, recoverable when lossy).
    pub applied: bool,
    /// Reduction as `(before - after) / before`; 0.0 whenever nothing applied.
    pub ratio: f64,
    pub before: TokenEstimate,
    pub after: TokenEstimate,
    /// Recovery handle for the original bytes, present only when the original
    /// was actually stored. A caller must never claim recoverability without it.
    pub handle: Option<String>,
}

/// Keeps original bytes addressable by handle. Implementations must return the
/// bytes byte-for-byte or nothing at all — a lost `get` must read as an unknown
/// handle, never as a guess.
pub trait RecoveryStore {
    /// Store `original` and return its handle. Idempotent: the handle is
    /// derived from the bytes, so storing twice returns the same handle.
    fn put(&mut self, original: &[u8]) -> String;
    /// The exact stored bytes, or `None` for an unknown handle.
    fn get(&self, handle: &str) -> Option<Vec<u8>>;
}

/// In-memory store for tests and ephemeral sessions. The persistent SQLite
/// store is a later revision; nothing in the engine may depend on which one is
/// wired, and passing `None` for the store disables all lossy emission rather
/// than crashing.
#[derive(Debug, Default)]
pub struct MemoryStore {
    blobs: BTreeMap<String, Vec<u8>>,
}

impl RecoveryStore for MemoryStore {
    fn put(&mut self, original: &[u8]) -> String {
        let handle = recovery_handle(original);
        self.blobs.insert(handle.clone(), original.to_vec());
        handle
    }

    fn get(&self, handle: &str) -> Option<Vec<u8>> {
        self.blobs.get(handle).cloned()
    }
}

/// Content-addressed handle: `"rcv_"` plus the hex of the first 8 bytes of the
/// SHA-256 of the original (64 collision bits, the same width the upstream
/// engine uses). Deriving the handle from the bytes is what makes storing
/// idempotent and retrieval verifiable: a handle can only name one payload, so
/// a bad recovery is always detectable as a mismatch, never silent.
#[must_use]
pub fn recovery_handle(original: &[u8]) -> String {
    let sum = sha256(original);
    let mut out = String::with_capacity(4 + 16);
    out.push_str("rcv_");
    for byte in &sum[..8] {
        out.push(char::from_digit((byte >> 4).into(), 16).unwrap());
        out.push(char::from_digit((byte & 0xf).into(), 16).unwrap());
    }
    out
}

/// The compression core. Construct with [`Engine::new`].
pub struct Engine {
    mode: Mode,
    counter: Box<dyn Counter>,
    compressors: BTreeMap<&'static str, Box<dyn Compressor>>,
    store: Option<Box<dyn RecoveryStore>>,
}

impl core::fmt::Debug for Engine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Engine")
            .field("mode", &self.mode)
            .field("counter", &self.counter.name())
            .field("compressors", &self.compressors.keys().collect::<Vec<_>>())
            .field("store", &self.store.is_some())
            .finish()
    }
}

impl Engine {
    #[must_use]
    pub fn new(
        mode: Mode,
        counter: Box<dyn Counter>,
        compressors: Vec<Box<dyn Compressor>>,
        store: Option<Box<dyn RecoveryStore>>,
    ) -> Self {
        Self {
            mode,
            counter,
            compressors: compressors
                .into_iter()
                .map(|c| (c.content_type(), c))
                .collect::<BTreeMap<_, _>>(),
            store,
        }
    }

    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Classify, compress, measure, and decide what to emit. Never fails.
    pub fn compress(&mut self, input: &[u8], opts: Options) -> Result {
        let detected = opts
            .content_type
            .unwrap_or_else(|| crate::detect::detect(input));
        let before = self.estimate(input);

        let Some(compressor) = self.compressors.get(detected.as_str()) else {
            return self.passthrough(detected, before, input);
        };

        let class = compressor.safety_class();
        // An unregistered safety class is impossible by construction (the enum
        // is closed and every variant is in the registry); lookup documents the
        // gate for future variants.
        let info = safety::lookup(class);

        let (out, ok) = compressor.compress(input);
        if !ok {
            return self.passthrough(detected, before, input);
        }
        let after = self.estimate(&out);
        if after.tokens >= before.tokens {
            // "Not actually smaller" claims nothing. This check is also what
            // lets a compressor report ok while returning its input.
            return self.passthrough(detected, before, input);
        }
        if self.mode == Mode::Record {
            // The dry run measured everything and still hands back the original.
            return Result {
                body: input.to_vec(),
                content_type: detected,
                applied: false,
                ratio: 0.0,
                before,
                after,
                handle: None,
            };
        }

        let mut handle = None;
        if info.requires_recovery {
            // Lossy bytes may only ship when the original is provably stored.
            let Some(store) = self.store.as_mut() else {
                return self.passthrough(detected, before, input);
            };
            let h = store.put(input);
            if store.get(&h).as_deref() != Some(input) {
                return self.passthrough(detected, before, input);
            }
            handle = Some(h);
        }

        Result {
            body: out,
            content_type: detected,
            applied: true,
            ratio: ratio(before.tokens, after.tokens),
            before,
            after,
            handle,
        }
    }

    /// The byte-exact original behind a handle, or `None` for an unknown one.
    pub fn retrieve(&self, handle: &str) -> Option<Vec<u8>> {
        self.store.as_ref()?.get(handle)
    }

    fn estimate(&self, bytes: &[u8]) -> TokenEstimate {
        TokenEstimate::new(self.counter.count(bytes), self.counter.name())
    }

    fn passthrough(&self, kind: ContentType, before: TokenEstimate, input: &[u8]) -> Result {
        let after = before.clone();
        Result {
            body: input.to_vec(),
            content_type: kind,
            applied: false,
            ratio: 0.0,
            before,
            after,
            handle: None,
        }
    }
}

/// `(before - after) / before`, 0.0 when `before` is 0.
#[must_use]
fn ratio(before: usize, after: usize) -> f64 {
    if before == 0 {
        return 0.0;
    }
    (before.saturating_sub(after)) as f64 / before as f64
}

/// SHA-256, inline, with no new dependency: the engine already trusts `regex`
/// and `serde` and this is the only digest it needs.
fn sha256(bytes: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (bytes.len() as u64) * 8;
    let mut msg = bytes.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, v) in [a, b, c, d, e, f, g, hh].into_iter().zip(h.iter_mut()) {
            *v = v.wrapping_add(slot);
        }
    }

    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Convenience constructor for tests and embedders: default counter, no
/// compressors registered, in-memory store.
#[must_use]
pub fn engine_with(mode: Mode, store: bool) -> Engine {
    Engine::new(
        mode,
        Box::new(tokens::ApproxCounter),
        Vec::new(),
        if store {
            Some(Box::<MemoryStore>::default())
        } else {
            None
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::Class;

    /// A pure shrinking transform used only to exercise the emit gate.
    struct Halver;
    impl Compressor for Halver {
        fn content_type(&self) -> &'static str {
            "text"
        }
        fn safety_class(&self) -> Class {
            Class::S4
        }
        fn compress(&self, input: &[u8]) -> (Vec<u8>, bool) {
            (vec![b'x'; input.len() / 2], true)
        }
    }

    fn engine(mode: Mode, store: bool) -> Engine {
        Engine::new(
            mode,
            Box::new(tokens::ApproxCounter),
            vec![Box::new(Halver)],
            if store {
                Some(Box::<MemoryStore>::default())
            } else {
                None
            },
        )
    }

    #[test]
    fn unknown_mode_fails_closed_to_record() {
        assert_eq!(Mode::parse("compresss"), Mode::Record);
        assert_eq!(Mode::parse(""), Mode::Record);
        assert_eq!(Mode::parse("COMPRESS"), Mode::Record);
        assert_eq!(Mode::parse("compress"), Mode::Compress);
    }

    #[test]
    fn no_compressor_registered_passes_the_original_through() {
        let mut e = engine_with(Mode::Compress, true);
        let r = e.compress(b"hello world hello world", Options::default());
        assert!(!r.applied);
        assert_eq!(r.body, b"hello world hello world");
        assert_eq!(r.ratio, 0.0);
        assert_eq!(r.handle, None);
    }

    #[test]
    fn record_mode_never_emits_compressed_bytes() {
        let payload = vec![b'a'; 400];
        let mut e = engine(Mode::Record, true);
        let r = e.compress(&payload, Options::default());
        assert_eq!(r.body, payload, "record mode must hand back the original");
        assert!(!r.applied);
        assert_eq!(r.handle, None, "record mode stores nothing");
    }

    #[test]
    fn lossy_result_without_a_store_never_emits() {
        let payload = vec![b'a'; 400];
        let mut e = engine(Mode::Compress, false);
        let r = e.compress(&payload, Options::default());
        assert_eq!(r.body, payload, "no store means no lossy emission");
        assert!(!r.applied);
    }

    #[test]
    fn lossy_result_with_a_store_emits_and_is_retrievable() {
        let payload = vec![b'a'; 400];
        let mut e = engine(Mode::Compress, true);
        let r = e.compress(&payload, Options::default());
        assert!(r.applied);
        assert!(r.ratio > 0.4, "ratio {r:?}");
        let handle = r.handle.expect("lossy result must carry a handle");
        assert_eq!(e.retrieve(&handle).as_deref(), Some(payload.as_slice()));
    }

    #[test]
    fn not_smaller_claims_nothing() {
        struct Grower;
        impl Compressor for Grower {
            fn content_type(&self) -> &'static str {
                "text"
            }
            fn safety_class(&self) -> Class {
                Class::S4
            }
            fn compress(&self, input: &[u8]) -> (Vec<u8>, bool) {
                let mut out = input.to_vec();
                out.extend_from_slice(b" and then some more words to be longer");
                (out, true)
            }
        }
        let mut e = Engine::new(
            Mode::Compress,
            Box::new(tokens::ApproxCounter),
            vec![Box::new(Grower)],
            Some(Box::<MemoryStore>::default()),
        );
        let r = e.compress(b"some text here", Options::default());
        assert!(!r.applied);
        assert_eq!(r.ratio, 0.0);
    }

    #[test]
    fn handle_is_content_addressed_and_idempotent() {
        let h1 = recovery_handle(b"same bytes");
        let h2 = recovery_handle(b"same bytes");
        let h3 = recovery_handle(b"different");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        assert!(h1.starts_with("rcv_"));
        assert_eq!(h1.len(), 4 + 16);
    }

    #[test]
    fn inline_sha256_matches_known_vectors() {
        // Guard for the hand-rolled digest: these are the first-16-byte hex
        // prefixes published by an independent SHA-256 implementation.
        assert_eq!(recovery_handle(b""), "rcv_e3b0c44298fc1c14");
        assert_eq!(recovery_handle(b"same bytes"), "rcv_58100dc8fc06562c");
        assert_eq!(recovery_handle(b"different"), "rcv_9d6f965ac832e40a");
        // 56-byte payload crosses the padding boundary at 64 bytes.
        let long = vec![b'a'; 56];
        assert_eq!(recovery_handle(&long), "rcv_b35439a4ac6f0948");
    }

    #[test]
    fn ratio_definition_matches_the_engine_contract() {
        assert_eq!(ratio(100, 67), 0.33);
        assert_eq!(ratio(0, 0), 0.0);
        assert_eq!(ratio(10, 10), 0.0);
    }
}
