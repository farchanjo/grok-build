# OpenRouter

OpenRouter is a first-class provider with its own native API surface. It is
**not** treated as a full OpenAI administration clone.

Grok Build's OpenRouter baseline is pinned by SHA-256
`90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203`. The local
baseline contains **69 paths** and **89 operations**. The generated client
contains **98 OpenRouter bindings** — the 89 primary operations plus nine
non-primary companion bindings (98 total, 89 primary).

```bash
grok provider set-key openrouter --from-env <OPENROUTER_KEY_ENV>
grok openrouter ops
grok openrouter key
grok openai --provider openrouter models list
```

- **Native-only surface.** OpenRouter-specific operations (per-instance
  preferences, credits, and catalog calls) are OpenRouter-native and are not
  an OpenAI administration clone. The OpenAI-compatible overlap is reported
  separately.
- **Per-instance preferences, credits, and catalog calls** are scoped to the
  selected instance and never cross accounts.
- Supported transports present in the generated bindings include HTTP JSON,
  SSE, multipart, and binary behavior.
- Mutating operations require confirmation (`--yes`), and `--dry-run` computes
  the operation without performing it.

Multiple OpenRouter instances coexist under distinct instance IDs; configured
catalog model IDs use `<instance>:<upstream-model-id>` and are never rebound to
a sibling. A slash may be part of the upstream OpenRouter slug, but it is not
the instance separator. Configured-instance credentials fail closed. See
[Multi-Account Providers](../user-guide/29-multi-account-providers.md),
[Retrieval and Prime](../user-guide/30-retrieval-and-prime.md), and
[Configuration](../user-guide/05-configuration.md).

Coding-agent backends: Chat Completions (default) and beta Responses
(`store = false`, no `previous_response_id`). Capability reports separate
OpenAI-compatible overlap from OpenRouter-native operations.
