# OpenRouter

OpenRouter is a first-class provider with its own native API surface. It is
**not** treated as a full OpenAI administration clone.

```bash
grok provider set-key openrouter --from-env OPENROUTER_API_KEY
grok openrouter ops
grok openrouter key
grok openai --provider openrouter models list
```

Coding-agent backends: Chat Completions (default) and beta Responses
(`store = false`, no `previous_response_id`). Capability reports separate
OpenAI-compatible overlap from OpenRouter-native operations.
