# Compatibility baselines and contracts

Grok Build pins **official API shape baselines** for OpenAI and OpenRouter so
compatibility claims stay auditable. OpenRouter is **not** a full OpenAI
platform clone.

## Terminology

| Term | Meaning |
| --- | --- |
| **Baseline** | Provenance-stamped inventory from an official OpenAPI document |
| **Supported / Unsupported / Unknown** | Tri-state claim status (`Unknown` fails closed) |
| **Baseline presence** | Operation exists in the official baseline inventory |
| **Client completeness** | Typed client binding coverage (Change 9+; **Unknown/NotImplemented** in Change 4) |
| **Intersection** | Declared OpenAI-compatible ops with **semantic** schema/doc evidence; transports/content types are the **common subset** of both vendors |
| **Same-path unverified** | METHOD+path on both sides without verified compatible semantics (not “native”) |
| **OpenRouter path-exclusive** | Path exclusive to OpenRouter (`openrouter_contract_outside_intersection`) |

## Inventories

| Inventory | Path |
| --- | --- |
| OpenAI (commit-pinned) | `baselines/openai/endpoint_inventory.json` |
| OpenRouter | `baselines/openrouter/endpoint_inventory.json` |
| Semantic intersection | `baselines/intersection/declared_intersection.json` |

## Domain model

Module: `xai_grok_inference::compatibility`

- Claim surfaces include `OpenaiBaselinePresence` vs `OpenaiClientCompleteness`
  (must not report Supported client completeness with NotImplemented bindings)
- Full OpenAI ledger: **287** baseline-presence + **287** client-completeness
  claims (`ProviderInventory::baseline_presence_claims` /
  `client_completeness_claims`); Change 4 completeness is always `Unknown`
- Each operation: method, path, multi-label **transports**, request/response
  content types
- OpenRouter endpoints partition into three disjoint categories covering the
  full baseline (one serialized source each; no duplicate native alias)

## Baseline update procedure

1. Download the **immutable** source (OpenAI: commit-addressed raw URL).
2. Record real UTC fetch time, SHA-256, byte size (and git SHA for OpenAI).
3. Run the provider generator with `--expect-source-sha256` / size checks.
4. Refresh the semantic intersection; re-validate partition coverage.
5. `cargo test -p xai-grok-inference compatibility openrouter_baseline`.

## Safety

Public API shape only. Unit tests never perform network I/O.
