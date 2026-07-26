# Anthropic (Claude)

Anthropic is a **first-class peer provider** in Grok Build. It is never the
global default or principal provider. Connect it only when you need Claude
models; xAI, OpenAI, OpenRouter, and custom Messages gateways remain
independent.

## Quick start (API key)

```bash
# Preferred: store the key from the environment (value is never printed)
export ANTHROPIC_API_KEY="…"   # set in your shell; do not commit
grok provider set-key anthropic --from-env ANTHROPIC_API_KEY

# Or in the TUI: /providers → Anthropic → Connect / Replace key
# Test / refresh catalog without starting a chat turn:
#   /providers → Anthropic → Test / Refresh
# Disconnect clears only the Anthropic vault entry and models cache:
grok provider disconnect anthropic
```

Environment-only fallback (no vault write):

```bash
export ANTHROPIC_API_KEY="…"
```

Keys live in the owner-only vault under `anthropic::api_key`. They are
**never** written to `config.toml` or `extra_headers`.

## Native API architecture

Grok Build uses a **repository-owned Rust HTTP client** against
`https://api.anthropic.com` with:

- `x-api-key` (never `Authorization: Bearer` for direct Anthropic)
- pinned `anthropic-version: 2023-06-01`
- optional `anthropic-beta` only when a feature explicitly requires it

This is **not** the Claude Code / Claude Agent binary, and not a shell-out to
Anthropic tooling. Grok owns tools, permissions, sandbox, MCP, hooks,
subagents, workflows, sessions, and compaction. Anthropic server tools,
Anthropic-hosted MCP connectors, and server-side compaction are **off by
default** and are not emitted on the wire unless a future opt-in path is
explicitly enabled.

Full user guide: [Anthropic provider](../user-guide/25-anthropic-provider.md).

## Supported endpoints (current client)

| Area | Status |
| --- | --- |
| Messages stream / non-stream | Supported (product agent path) |
| Models list / retrieve | Supported (catalog + cache) |
| `count_tokens` | Supported (client library) |
| Files API (`files-api-2025-04-14` beta) | **Library only** — see below |
| Batches | Deferred |
| Admin API | Deferred |
| Managed Agents | Deferred |

### Files API scope (not a product command)

Upload / list / retrieve / delete live on the **repository-owned Rust client**
(`xai-grok-inference` Anthropic module) and are covered by **mock HTTP unit
tests**. There is **no** TUI command, `grok` CLI surface, or agent auto-upload
that calls Files in ordinary product use. A live product integration is
**deferred**. Docs that mention delete describe the **client library API**
(callers can delete by file id), not a user-facing Grok slash command.

## Models and picker

With an Anthropic key configured, the model picker includes curated presets
(for example `anthropic-claude-sonnet-5`, `anthropic-claude-opus-5`,
`anthropic-claude-haiku-4-5`) and account-visible Models API entries as
`anthropic:<upstream-id>` (experimental tooling assumptions). Catalog cache:
owner-only `anthropic_models_cache.json`. Disconnect removes that cache only.

## Custom Messages / OpenRouter Claude

- **Custom Messages** (`api_backend = "messages"` on a non-Anthropic provider)
  remains a generic protocol path — it does **not** inherit direct Anthropic
  identity headers solely because the wire protocol is Messages.
- **OpenRouter Claude** (`openrouter:anthropic/...`) is OpenRouter identity and
  billing, not the Anthropic peer.

## Claude Agent CLI (experimental, gated)

Subscription-backed Claude Agent CLI is **experimental**, off in default and
`release-dist` builds, and requires:

1. compile feature `claude-cli-runtime` (not in ordinary releases),
2. env `GROK_CLAUDE_CLI_RUNTIME=1` (or `true` / `yes` / `on`),
3. a successful official `claude` binary probe.

Ordinary release builds do not show or select the subscription CLI card.
Release availability is **pending authorization**. See the user guide for the
capability matrix, safe-mode, permission bridge, and cross-mode session rules.

## Ops

```bash
# After set-key / disconnect, catalog rebuilds for the open model picker
grok provider set-key anthropic --from-env ANTHROPIC_API_KEY
grok provider disconnect anthropic
```

See also: [OpenRouter](openrouter.md), [OpenAI Platform](openai-platform.md),
[migration](../user-guide/26-anthropic-migration.md).
