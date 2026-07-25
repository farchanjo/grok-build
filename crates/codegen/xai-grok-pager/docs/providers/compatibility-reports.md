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

Terminology: **client completeness** (Grok implements the endpoint) is
distinct from a **configured provider's** Supported/Unsupported/Unknown
capability. `solaris` is development-only.
