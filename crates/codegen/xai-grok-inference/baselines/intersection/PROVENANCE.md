# Declared OpenAI ↔ OpenRouter intersection provenance

This directory pins the **explicit** OpenAI-compatible intersection between the
checked-in OpenAI and OpenRouter baseline inventories.

## Why declared (not path-guessed)

Matching `METHOD + path` alone is a *candidate filter*, not an identity claim.
Each intersection member records:

- a stable `shared_id`
- both vendors' `operationId` values (which often differ)
- method, path, API family, transport, content types
- baseline version + content SHA for both sides
- honest `client_binding` / `cli_binding` status (`not_implemented` in Change 4)

OpenRouter-native operations (present only on OpenRouter) are listed in
`openrouter_native_operations` and are **never** claimed as OpenAI platform
surface.

## Inputs

| Baseline | Provider | `info.version` | Content SHA-256 | Endpoints |
| --- | --- | --- | --- | --- |
| [`../openai/endpoint_inventory.json`](../openai/endpoint_inventory.json) | openai | 2.3.0 | `b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da` | see inventory |
| [`../openrouter/endpoint_inventory.json`](../openrouter/endpoint_inventory.json) | openrouter | 1.0.0 | `90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203` | see inventory |

Generated at: `2026-07-25T17:00:00Z` (UTC).

## File

- [`declared_intersection.json`](declared_intersection.json)

## Claims policy

- **OpenAI client completeness** is measured against the OpenAI baseline only.
- **OpenRouter-native coverage** is measured against OpenRouter endpoints that
  are not intersection members.
- **Configured-provider capability claims** may remain `Unknown` without
  reducing either completeness metric.
- Typed client/CLI bindings are **not implemented** in Change 4; statuses are
  stored and tested as `not_implemented`, not as fake coverage.

## Validation

```sh
cargo test -p xai-grok-inference compatibility -- --nocapture
```

Unit tests require every declared member to resolve unambiguously in **both**
baseline inventories and reject duplicate `shared_id` values.
