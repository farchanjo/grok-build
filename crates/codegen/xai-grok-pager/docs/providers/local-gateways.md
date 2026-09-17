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

## Session affinity and prompt caching

Grok follows a session on a self-hosted endpoint automatically. There is no flag
to enable: every request built from a session carries that session's id, and the
provider-specific carriers are chosen from the provider identity.

| Server | What is sent | What it does |
| --- | --- | --- |
| SGLang | `session_id` and `bootstrap_room` in the body | `bootstrap_room` drives `--load-balance-method follow_bootstrap_room` (`rank = bootstrap_room % dp_size`), so a session keeps landing on the rank that holds its prefix. With `--enable-session-radix-cache` (+ `SGLANG_ENABLE_UNIFIED_RADIX_TREE=1`) `session_id` additionally registers the session's KV, so an idle conversation is evicted after unrelated KV. Grok posts `/close_session` at session end to release those references. |
| vLLM | `session_id` in the body and the `X-Session-ID` header | `session_id` is vLLM's stable session identity and is reported on KV-cache events, so a KV-events-aware router can pin the session. `bootstrap_room` is ignored (`extra="allow"`). |
| Generic proxy / other | `session_id` and `bootstrap_room` | Read them if you route on the session; unknown body fields are otherwise ignored. |

Both fields are derived from the session id, so a resumed session lands on the
same rank and reuses its prefix cache after a client restart.

**Without a session radix cache** the behavior degrades cleanly: SGLang accepts
`session_id` and ignores it, `bootstrap_room` still pins the rank, and the
ordinary prefix cache (RadixAttention, on by default) keeps the session's prefix
warm on that rank. vLLM behaves the same way with its own prefix cache enabled
(`--enable-prefix-caching`). No configuration change is needed for either mode.

`dialect = "vllm"` / `"sglang"` is **not** required for the session fields; it only
selects the reasoning-key and reasoning-echo wire shaping. Declare it when your
server needs that shaping.

`session_id` must stay at most 256 characters (OpenRouter's limit; the others are
more permissive). Grok session ids are well under it.
