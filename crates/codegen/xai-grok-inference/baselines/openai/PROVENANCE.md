# OpenAI API baseline provenance

This directory pins a **compact endpoint and field inventory** derived from
OpenAI's official OpenAPI description (`openai/openai-openapi`). The full
OpenAPI YAML (~2.8 MiB as of the pin date) is **not** vendored.

## Source

| Field | Value |
| --- | --- |
| Provider | OpenAI |
| Official docs | https://platform.openai.com/docs/api-reference |
| OpenAPI repository | https://github.com/openai/openai-openapi |
| OpenAPI YAML (pin URL) | https://raw.githubusercontent.com/openai/openai-openapi/master/openapi.yaml |
| OpenAPI version field | `3.1.0` |
| Document `info.version` | `2.3.0` |
| Document `info.title` | OpenAI API |
| Source format pinned | YAML |
| Pin date (UTC) | 2026-07-25T17:00:00Z |
| Content SHA-256 (YAML blob) | `b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da` |
| Content size (bytes) | `2827615` |
| Inventory file | [`endpoint_inventory.json`](endpoint_inventory.json) |
| Generator | [`generate_inventory.py`](generate_inventory.py) |
| License note | OpenAPI description from `openai/openai-openapi` (MIT). Compact inventory is derived public shape metadata only. |

The SHA-256 and size describe the **exact local OpenAPI YAML blob** used to
build the inventory. They do **not** describe the compact inventory file.

Path templates in this OpenAPI pin **omit** the `/v1` prefix; clients append
`/v1` via the API base URL (for example `https://api.openai.com/v1`).

## What is checked in

- `endpoint_inventory.json` — every path/method, optional content types and
  transport labels, coding-agent schema field inventories, priority endpoints.
- `generate_inventory.py` — deterministic generator (priority endpoints, schema
  allowlist, stable ordering, canonical JSON).
- `fixtures/mini_openapi.json` — offline generator fixture (no network).
- `PROVENANCE.md` — this file.

## Exact regeneration command

Download once (operator machine only; never in CI unit tests):

```sh
curl -fsSL \
  "https://raw.githubusercontent.com/openai/openai-openapi/master/openapi.yaml" \
  -o /tmp/openai-openapi.yaml
shasum -a 256 /tmp/openai-openapi.yaml
wc -c /tmp/openai-openapi.yaml
```

Convert YAML → JSON (dates become ISO strings), then regenerate:

```sh
python3 - <<'PY'
import json, yaml
from datetime import date, datetime
from pathlib import Path

def default(o):
    if isinstance(o, (date, datetime)):
        return o.isoformat()
    raise TypeError(type(o))

data = yaml.safe_load(Path("/tmp/openai-openapi.yaml").read_bytes())
Path("/tmp/openai-openapi.json").write_text(json.dumps(data, default=default))
PY

cd crates/codegen/xai-grok-inference/baselines/openai
python3 generate_inventory.py \
  --input /tmp/openai-openapi.json \
  --output endpoint_inventory.json \
  --fetched-at-utc 2026-07-25T17:00:00Z \
  --source-sha256 b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da \
  --source-bytes 2827615 \
  --source-format yaml
```

Validate:

```sh
python3 generate_inventory.py \
  --input /tmp/openai-openapi.json \
  --output endpoint_inventory.json \
  --fetched-at-utc 2026-07-25T17:00:00Z \
  --source-sha256 b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da \
  --source-bytes 2827615 \
  --source-format yaml \
  --check
```

When re-pinning, update SHA-256, size, `fetched_at_utc`, and this table.

## Validation (no network)

```sh
export GROK_HOME="${HOME}/.grokdev"
export GROK_LEADER_SOCKET="${GROK_HOME}/leader.sock"
export GROK_DISABLE_AUTOUPDATER=1
cargo test -p xai-grok-inference compatibility -- --nocapture
```

## Intentional exclusions

- Full OpenAPI YAML/JSON is not vendored (size).
- Realtime WebSocket protocol details beyond HTTP path ops are not expanded.
- Vendor-private experimental ops not present in the official OpenAPI are
  omitted.
- This baseline is **OpenAI platform shape**, not OpenRouter. OpenRouter is
  measured separately and is never claimed to equal the full OpenAI platform.

## Safety

The inventory contains only public API shape data. It never includes API keys,
prompts, or response bodies.
