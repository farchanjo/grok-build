# OpenRouter API baseline provenance

Compact endpoint inventory derived from OpenRouter's official OpenAPI document.
Full OpenAPI (~1.6 MiB) is not vendored.

## Source pin

| Field | Value |
| --- | --- |
| Provider | OpenRouter |
| Official docs | https://openrouter.ai/docs/api_reference/overview |
| OpenAPI JSON | https://openrouter.ai/openapi.json |
| OpenAPI YAML | https://openrouter.ai/openapi.yaml |
| OpenAPI version field | `3.1.0` |
| Document `info.version` | `1.0.0` |
| Inventory | [`endpoint_inventory.json`](endpoint_inventory.json) |
| Generator | [`generate_inventory.py`](generate_inventory.py) |
| **Content SHA-256** | `90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203` |
| Content size (bytes) | `1653634` |
| **Fetched at (UTC)** | `2026-07-25T16:25:35Z` |

(Re-verified 2026-07-25: live `openapi.json` still matches this SHA.)

## Regeneration

```sh
curl -fsSL "https://openrouter.ai/openapi.json" -o /tmp/openrouter-openapi.json
shasum -a 256 /tmp/openrouter-openapi.json

cd crates/codegen/xai-grok-inference/baselines/openrouter
python3 generate_inventory.py \
  --input /tmp/openrouter-openapi.json \
  --output endpoint_inventory.json \
  --fetched-at-utc 2026-07-25T16:25:35Z \
  --expect-source-sha256 90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203 \
  --expect-source-bytes 1653634
```

## Contract fields (format_version 2)

Same transport/content-type contract as the OpenAI baseline (see shared
`baselines/_shared/openapi_contract.py`).

## How Grok Build uses the inventory

- `xai-grok-inference::openrouter_baseline` loads the inventory and asserts
  integrity.
- `xai-grok-inference::compatibility` reuses this inventory with the OpenAI
  baseline and the **semantic** intersection under `baselines/intersection/`.
- OpenRouter is **not** a full OpenAI platform clone.

## Safety

Public API shape only. No API keys, prompts, or response bodies.
