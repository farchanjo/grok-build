# Fuzzing xai-grok-tools media preprocessing

Coverage-guided fuzzing for the native media preprocessing layer using [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) (libFuzzer).

## Prerequisites

```bash
cargo install cargo-fuzz   # if not already installed
rustup toolchain install nightly
```

## Targets

| Target | What it fuzzes |
|---|---|
| `decode` | Native FFmpeg decode (`DecodeSession`: probe / next-frame / frame-at-seconds / audio PCM) plus content-only request and source deserialization |

## Build-time gating

- The native arm of `decode` fuzzes the public `xai-grok-tools::media::ffmpeg`
  API **only** when compatible FFmpeg 8 headers are discoverable via
  pkg-config **and** `GROK_DISABLE_MEDIA_FFMPEG` is unset — the same gate the
  tools crate's own `build.rs` applies. The harness `build.rs` mirrors that
  discovery and emits a crate-local `media_ffmpeg` cfg.
- Without headers (or with the kill switch set), the native arm compiles out
  and the target still fuzzes the always-on deserialization surface.
- Every native iteration runs under hard bounds: source bytes, pixels,
  dimensions, duration, frame count, PCM samples, frame bytes, and a
  per-request wall-clock deadline (`request_timeout_ms`).

## Running

From `crates/codegen/xai-grok-tools`:

```bash
# Run indefinitely (Ctrl-C to stop):
cargo +nightly fuzz run decode fuzz/corpus/decode fuzz/seeds/decode -- -max_len=262144

# Run for 5 minutes:
cargo +nightly fuzz run decode fuzz/corpus/decode fuzz/seeds/decode -- -max_len=262144 -max_total_time=300
```

- `corpus/` — auto-generated inputs (gitignored)
- `seeds/` — hand-written seed inputs (checked in)

Notes:

- `cargo fuzz run` builds in release mode by default. The tools crate's
  `build.rs` bundles ripgrep in release builds (a one-time download, or set
  `GROK_TOOLS_BUNDLE_RG_PATH` to a local binary for offline builds).
- libFuzzer enables ASan/UBSan by default; the FFmpeg 8 sanity model
  (plan 10.5) is intentionally *not* crash-isolated, so a native decoder
  defect can abort the fuzzer process — that is exactly the signal these
  runs are meant to surface.

## Reproducing a crash

When a crash is found, the input is saved to `artifacts/decode/crash-<hash>`. Reproduce it with:

```bash
cargo +nightly fuzz run decode fuzz/artifacts/decode/crash-<hash>
```

## Adding seed inputs

Drop binary fixtures (PNG / AVI / WAV) or text files into `seeds/decode/`.
Binary seeds such as a real PNG, a hand-rolled AVI (see the decode unit tests
in the tools crate for the fixture builder), and a WAV reach the native
decoders directly and help the fuzzer find deeper code paths faster.
