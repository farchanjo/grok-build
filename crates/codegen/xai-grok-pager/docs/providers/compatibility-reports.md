# Compatibility reports

Machine-readable coverage is generated from pinned baselines:

| Report | Source |
| --- | --- |
| OpenAI baseline | `xai-grok-inference/baselines/openai/endpoint_inventory.json` |
| OpenRouter baseline | `xai-grok-inference/baselines/openrouter/endpoint_inventory.json` |
| Semantic intersection | `xai-grok-inference/baselines/intersection/declared_intersection.json` |
| Client/CLI bindings | `xai-grok-inference/baselines/operation_bindings_report.json` |

```bash
grok provider capabilities openai --json
grok openai --provider openai ops --json
grok openrouter ops
```

Anthropic peer setup and endpoint status (Messages, Models, count_tokens,
Files; Batches/Admin/Managed Agents deferred) are documented in
[anthropic.md](anthropic.md) and the user guide
[25-anthropic-provider.md](../user-guide/25-anthropic-provider.md). Anthropic
does not currently ship a separate OpenAPI inventory under
`baselines/anthropic/`; pin checks live in the repository-owned client tests
(`xai-grok-inference` Anthropic module, version `2023-06-01`, Files beta
`files-api-2025-04-14`).

Terminology: **client completeness** (Grok implements the endpoint) is
distinct from a **configured provider's** Supported/Unsupported/Unknown
capability. `solaris` is development-only.
