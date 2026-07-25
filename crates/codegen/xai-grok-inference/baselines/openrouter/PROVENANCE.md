# OpenRouter API baseline provenance

This directory pins a **compact endpoint and field inventory** derived from
OpenRouter's official OpenAPI document. The full OpenAPI payload is not
vendored (~1.6 MiB as of the pin date) so the repository stays lean while still
providing a deterministic contract surface for conformance work.

## Source

| Field | Value |
| --- | --- |
| Provider | OpenRouter |
| Official docs | https://openrouter.ai/docs/api_reference/overview |
| OpenAPI JSON | https://openrouter.ai/openapi.json |
| OpenAPI YAML | https://openrouter.ai/openapi.yaml |
| OpenAPI version field | `3.1.0` |
| Document `info.version` | `1.0.0` |
| Document `info.title` | OpenRouter API |
| Inventory file | [`endpoint_inventory.json`](endpoint_inventory.json) |
| Generator | [`generate_inventory.py`](generate_inventory.py) |
| Pin date (UTC) | 2026-07-24T22:27:00Z |
| Content SHA-256 | `90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203` |
| Content size (bytes) | `1653634` |

The SHA-256 and size describe the **exact local OpenAPI blob** used to build
the inventory (same values are embedded in `endpoint_inventory.json` under
`baseline`). They do **not** describe the compact inventory file.

## What is checked in

- `endpoint_inventory.json` — every path/method (`operationId`, tags, summary),
  plus field inventories for coding-agent-relevant schemas and the priority
  endpoint list used by tests.
- `generate_inventory.py` — deterministic generator (pinned schema allowlist,
  priority endpoints, stable ordering, canonical JSON).
- `fixtures/mini_openapi.json` — tiny offline OpenAPI fixture for generator
  tests (no network).
- `PROVENANCE.md` — this file.

## Exact regeneration command

Download once (operator machine only; never in CI unit tests):

```sh
curl -fsSL "https://openrouter.ai/openapi.json" -o /tmp/openrouter-openapi.json
```

Regenerate from that local file:

```sh
cd crates/codegen/xai-grok-inference/baselines/openrouter
python3 generate_inventory.py \
  --input /tmp/openrouter-openapi.json \
  --output endpoint_inventory.json \
  --fetched-at-utc 2026-07-24T22:27:00Z
```

Validate that the checked-in inventory matches the generator for a local blob:

```sh
python3 generate_inventory.py \
  --input /tmp/openrouter-openapi.json \
  --output endpoint_inventory.json \
  --fetched-at-utc 2026-07-24T22:27:00Z \
  --check
```

When re-pinning to a newer OpenAPI document, update `fetched_at_utc` and this
table's SHA-256 / size to match the new blob, then re-run the generator.

## Validation (no network)

```sh
export GROK_HOME="${HOME}/.grokdev"
export GROK_LEADER_SOCKET="${GROK_HOME}/leader.sock"
export GROK_DISABLE_AUTOUPDATER=1
cargo test -p xai-grok-inference openrouter_baseline -- --nocapture
cargo test -p xai-grok-inference openrouter_regression -- --nocapture
```

Unit tests never fetch OpenAPI over the network. Optional regeneration check:

```sh
OPENROUTER_OPENAPI_PATH=/tmp/openrouter-openapi.json \
  cargo test -p xai-grok-inference openrouter_inventory_regenerates -- --nocapture
```

## How Grok Build uses the inventory

- `xai-grok-inference::openrouter_baseline` loads the inventory and asserts
  integrity (`field_count`, unique paths/schemas, priority endpoints exist).
- `xai-grok-inference::compatibility` reuses this inventory alongside the
  OpenAI baseline and the declared intersection under
  `baselines/intersection/` (Change 4).
- Later milestones compare Grok serializers and capability claims against these
  endpoints and schema fields. OpenRouter is **not** treated as a full OpenAI
  clone; OpenRouter-native coverage is measured separately.

## Safety

The inventory contains only public API shape data. It never includes API keys,
prompts, or response bodies.
