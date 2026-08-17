# Platform client generation and operation metadata

## Operation metadata (PR10, preferred for coverage checks)

`generate_operation_metadata.py` unifies OpenAI/OpenRouter operation metadata.

**Authoritative inputs** (never circular):

- `baselines/{openai,openrouter}/endpoint_inventory.json`
- `src/openai_platform/generated/*_ops.rs` (parsed method specs)

**Derived caches** (regenerated, never used as inputs):

- `baselines/operation_table.json`
- `baselines/operation_bindings_report.json`
- `src/openai_platform/generated/bindings.rs`
- `xai-grok-shell/src/cli/generated_ops.rs`
- `xai-grok-shell/src/cli/typed_dispatch_runtime.rs`

```sh
# Validate inventories, ops, and derived artifacts (no network).
python3 crates/codegen/xai-grok-inference/baselines/scripts/generate_operation_metadata.py --check

# Repair transport gaps (skill zip binary, SSE companions) and regenerate caches.
python3 crates/codegen/xai-grok-inference/baselines/scripts/generate_operation_metadata.py --write
```

`--check` rejects missing/extra primaries, operation-id drift, inventory
transport vs op-mode mismatches (binary/multipart/websocket/SSE companions),
duplicate names, stale generated artifacts (including full
`typed_dispatch_runtime.rs` byte-compare after rustfmt), and per-arm swapped
client methods (self-test fixture). Rustfmt uses the repository `rustfmt.toml`.
Truthful counts: **287** OpenAI primaries, **89** OpenRouter primaries, **20**
SSE companions, **7** binary primaries.

## Full schema client generation (requires pinned OpenAPI blobs)

`generate_platform_client.py` regenerates typed ops/types from exact local
copies of the pinned OpenAPI documents (no network):

- OpenAI: `/tmp/openai-baseline-pin/openapi.yaml` (SHA-256
  `b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da`)
- OpenRouter: `/tmp/openrouter-baseline-pin/openapi.json` (or
  `/tmp/openrouter-openapi.json`; SHA-256
  `90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203`)

```sh
python3 crates/codegen/xai-grok-inference/baselines/scripts/generate_platform_client.py
# Then refresh derived metadata:
python3 crates/codegen/xai-grok-inference/baselines/scripts/generate_operation_metadata.py --write
```

No API credentials or live inference are used.
