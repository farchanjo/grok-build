# Z.ai Model API

Profile id: `zai-model-api`
Base URL: `https://api.z.ai/api/paas/v4` (general API; Coding Plan is a separate
configured instance if needed)

```bash
grok provider set-key zai-model-api --from-env ZAI_API_KEY
# or in TUI: /providers → Z.ai Model API
```

Agent backend: Chat Completions with Z.ai extensions (`thinking`,
`reasoning_content`, `tool_stream`) identity-gated to the Z.ai profile. Local
built-in and MCP tools run through Grok's permission/sandbox gates. Native Z.ai
web search / remote MCP stay off by default.

Never place API keys in TOML fixtures, logs, or reports. Conformance uses
`GROK_TEST_ZAI_API_KEY` only for ignored manual suites.
