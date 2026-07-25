# OpenAI API baseline provenance

Compact endpoint inventory derived from OpenAI's official OpenAPI description
at an **immutable git commit**. Full OpenAPI YAML (~2.8 MiB) is not vendored.

## Immutable source pin

| Field | Value |
| --- | --- |
| Provider | OpenAI |
| Repository | https://github.com/openai/openai-openapi |
| **Source revision (full SHA)** | `5c044be3bf3a42854e99e34616564eeb2124a317` |
| **Immutable raw URL** | https://raw.githubusercontent.com/openai/openai-openapi/5c044be3bf3a42854e99e34616564eeb2124a317/openapi.yaml |
| Official docs | https://platform.openai.com/docs/api-reference |
| OpenAPI version field | `3.1.0` |
| Document `info.version` | `2.3.0` |
| Source format | YAML |
| **Content SHA-256 (YAML)** | `b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da` |
| Content size (bytes) | `2827615` |
| **Fetched at (UTC)** | `2026-07-25T16:25:32Z` |
| Inventory | [`endpoint_inventory.json`](endpoint_inventory.json) |
| Generator | [`generate_inventory.py`](generate_inventory.py) |
| License | OpenAPI from openai/openai-openapi (MIT). Compact inventory is derived shape metadata only. |

The mutable branch path `.../master/openapi.yaml` is **not** used as the pin URL.

## Regeneration (operator machine)

```sh
curl -fsSL \
  "https://raw.githubusercontent.com/openai/openai-openapi/5c044be3bf3a42854e99e34616564eeb2124a317/openapi.yaml" \
  -o /tmp/openai-openapi.yaml
shasum -a 256 /tmp/openai-openapi.yaml   # must equal b58d6cd…
wc -c /tmp/openai-openapi.yaml           # must equal 2827615

cd crates/codegen/xai-grok-inference/baselines/openai
python3 generate_inventory.py \
  --source-yaml /tmp/openai-openapi.yaml \
  --output endpoint_inventory.json \
  --fetched-at-utc 2026-07-25T16:25:32Z \
  --expect-source-sha256 b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da \
  --expect-source-bytes 2827615

# Integrity check (byte-identical inventory):
python3 generate_inventory.py \
  --source-yaml /tmp/openai-openapi.yaml \
  --output endpoint_inventory.json \
  --fetched-at-utc 2026-07-25T16:25:32Z \
  --expect-source-sha256 b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da \
  --expect-source-bytes 2827615 \
  --check
```

The generator refuses to write when the YAML SHA/size does not match the expected pin.

Optional unit test when the blob is present:

```sh
OPENAI_OPENAPI_YAML_PATH=/tmp/openai-openapi.yaml \
  cargo test -p xai-grok-inference openai_inventory_regenerates -- --nocapture
```

## Contract fields (format_version 2)

Each operation records:

- `request_content_types` / `response_content_types` (validated media types)
- `transports` — multi-label set: `http_json`, `http_sse`, `http_multipart`,
  `http_binary`, `websocket`, or `unknown` only when OpenAPI cannot establish it

Stream-flag request schemas contribute **both** `http_json` and `http_sse`.
Binary responses are never collapsed to sole `http_json`.

## Safety

Public API shape only. No API keys, prompts, or response bodies.
