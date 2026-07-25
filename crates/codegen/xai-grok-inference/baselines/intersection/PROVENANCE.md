# Declared OpenAI ↔ OpenRouter intersection provenance

## Semantic membership (not path partition)

Intersection members require **explicit OpenAI-compatible semantics** evidenced
by official schemas/docs — not METHOD+path coincidence alone.

### Members (4)

| shared_id | METHOD path | Evidence |
| --- | --- | --- |
| `compat.createChatCompletion` | POST /chat/completions | CreateChatCompletionRequest / ChatRequest |
| `compat.createResponse` | POST /responses | CreateResponse / ResponsesRequest |
| `compat.createEmbedding` | POST /embeddings | CreateEmbeddingRequest + path |
| `compat.listModels` | GET /models | ListModelsResponse / ModelsListResponse |

Files, audio, video, and other same-path resources remain in
**same_path_unverified_overlap** when schemas may differ.

## OpenRouter baseline partition (disjoint, exact cover)

Every OpenRouter endpoint is in exactly one category:

1. **compatible intersection** (`members`)
2. **same_path_unverified_overlap** (METHOD+path also on OpenAI, semantics not verified)
3. **openrouter_contract_outside_intersection** (path exclusive to OpenRouter)

## Baseline pins

| Baseline | Content SHA-256 | Fetched at UTC | Endpoints |
| --- | --- | --- | --- |
| OpenAI (`source_revision` `5c044be3…`) | `b58d6cd94c881bdfd6a940bdc4db009e2c9b455accf8fd6a8b712458bc30c0da` | `2026-07-25T16:25:32Z` | 287 |
| OpenRouter | `90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203` | `2026-07-25T16:25:35Z` | 89 |

Generated at: `2026-07-25T16:25:35Z`.

## Claims

- Baseline presence ≠ client completeness.
- `client_binding` / `cli_binding` are `not_implemented` in Change 4.
