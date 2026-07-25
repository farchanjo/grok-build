# Compatibility baselines and contracts

Grok Build pins **official API shape baselines** for OpenAI and OpenRouter so
compatibility claims stay auditable. OpenRouter is **not** treated as a full
OpenAI platform clone.

## Terminology

| Term | Meaning |
| --- | --- |
| **Baseline** | A pinned, provenance-stamped inventory derived from an official OpenAPI document |
| **Supported** | Explicitly present and accepted for the named claim surface |
| **Unsupported** | Explicitly out of scope for the named claim surface |
| **Unknown** | No evidence yet; fail closed for capability claims, never invent coverage |
| **Intersection** | Declared OpenAI-compatible operations present in **both** baselines with explicit identities |
| **OpenRouter-native** | Operations only on OpenRouter (not in the OpenAI baseline) |
| **Client completeness** | How much of the OpenAI baseline a typed client binds (later milestones) |
| **Provider capability claim** | What a configured third-party provider advertises; may stay Unknown |

## Inventories (checked in)

| Inventory | Path |
| --- | --- |
| OpenAI baseline | `baselines/openai/endpoint_inventory.json` |
| OpenRouter baseline | `baselines/openrouter/endpoint_inventory.json` |
| Declared intersection | `baselines/intersection/declared_intersection.json` |

Provenance and regeneration steps live next to each inventory (`PROVENANCE.md`).

## Domain model (Rust)

Module: `xai_grok_inference::compatibility`

- Tri-state [`CompatibilityStatus`]: `Supported` / `Unsupported` / `Unknown`
- [`Evidence`] with kind, source, timestamp, baseline version
- [`OperationIdentity`]: family, operation id, method, path, transport, content type
- Separate claim surfaces: OpenAI client completeness, OpenRouter-native coverage,
  configured-provider capability
- Binding status for client/CLI: `Implemented` / `NotImplemented` / `Unknown`
  (Change 4 stores and tests `NotImplemented` only)

Unknown or additive enum values deserialize safely where serde defaults apply;
unknown transport/family labels map to `Unknown` / `Other` rather than panicking
at the claim layer.

## Baseline update procedure

1. Download the official OpenAPI document on an operator machine (never in unit tests).
2. Record SHA-256, byte size, and UTC fetch timestamp.
3. Run the provider-specific `generate_inventory.py` with those pin fields.
4. Refresh the declared intersection when either baseline changes (every member
   must still resolve in both inventories).
5. Run `cargo test -p xai-grok-inference compatibility`.

## Auditable claims

Any claim about “supports X” must cite:

- baseline provider + document version
- content SHA-256 of the source pin
- evidence kind and timestamp
- claim surface (client completeness vs OpenRouter-native vs provider capability)

A provider may remain **Unknown** for capability without lowering OpenAI client
completeness scores.

## Safety

Inventories contain public API shape only. No API keys, prompts, or response
bodies. Unit tests never perform network I/O.
