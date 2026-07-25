# Local and gateway OpenAI-compatible providers

Verified development patterns (not shipping defaults):

| Stack | Typical base URL | Notes |
| --- | --- | --- |
| vLLM | `http://127.0.0.1:8000/v1` | Mark generative paths only when the model is generative |
| SGLang | `http://127.0.0.1:30000/v1` | Tool/reasoning parsers are server-side |
| llama.cpp server | `http://127.0.0.1:8000/v1` | OpenAI-compatible chat |
| Ollama | `http://127.0.0.1:11434/v1` | Capability varies by model |
| LM Studio | `http://127.0.0.1:1234/v1` | Local desktop server |
| Azure OpenAI | resource-specific | Use `openai_compatible` with Azure base + headers |
| Generic reverse proxy | user-defined | Prefer explicit capability overrides |

`solaris` host endpoints are **development-only** conformance targets and are
never written as user defaults. See the ignored harness under
`xai_grok_shell::conformance::solaris`.
