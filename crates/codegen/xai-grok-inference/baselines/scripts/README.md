# Platform client generation

`generate_platform_client.py` is the canonical offline schema-derived
generator. Run it from the repository root against exact local copies of the
pinned OpenAPI documents:

- OpenAI: `/tmp/openai-baseline-pin/openapi.json` (SHA-256
  `b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da`)
- OpenRouter: `/tmp/openrouter-baseline-pin/openapi.json` (or
  `/tmp/openrouter-openapi.json`)

```sh
python3 crates/codegen/xai-grok-inference/baselines/scripts/generate_platform_client.py
```

Regeneration emits:

- `src/openai_platform/generated/*`
- `baselines/operation_bindings_report.json`
- `xai-grok-shell/src/cli/generated_ops.rs` and `typed_dispatch_runtime.rs`

No API credentials or live inference are used.
