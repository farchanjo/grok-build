# Anthropic Provider

Anthropic (Claude) is a first-class **peer** provider in Grok Build. It is
never the global default model source and never the principal account that
gates TUI startup. Connect Anthropic when you want Claude models; leave it
disconnected when you do not.

This guide covers:

1. Anthropic **API key** peer setup (`/providers`, vault, models cache)
2. **Native API** architecture and supported endpoints
3. Structured output, thinking, Files, billing/rate limits
4. Custom Messages gateways vs OpenRouter Claude
5. Experimental **Claude Agent CLI** subscription mode (gated)
6. Security, status messages, and troubleshooting

Related:

- Short provider page: [docs/providers/anthropic.md](../providers/anthropic.md)
- Migration: [27-anthropic-migration.md](27-anthropic-migration.md)
- Custom models / Messages TOML: [11-custom-models.md](11-custom-models.md)

---

## 1. Connect Anthropic (API key)

### Preferred: vault via environment

```bash
# Set the key in your shell profile or secret manager — do not commit it
export ANTHROPIC_API_KEY="…"
grok provider set-key anthropic --from-env ANTHROPIC_API_KEY
```

`set-key --from-env` reads the named environment variable and stores the value
in the owner-only vault (`auth.json` scope `anthropic::api_key`). The CLI does
**not** print the key.

### TUI

1. Open `/providers`
2. Select **Anthropic**
3. **Connect** / **Replace key**, **Test**, **Refresh**, or **Disconnect**

### Disconnect

```bash
grok provider disconnect anthropic
```

Disconnect clears only the Anthropic vault entry and the owner-only
`anthropic_models_cache.json`. Other providers (xAI, OpenAI, OpenRouter, …)
are untouched.

### Environment-only (no vault write)

```bash
export ANTHROPIC_API_KEY="…"
```

Grok can use the process environment when the vault entry is absent. Prefer
the vault for interactive use so keys are not required in every shell.

### Never put keys in config

**Do not** put Anthropic API keys in:

- `config.toml` `api_key` fields (discouraged for all providers; forbidden for
  product Anthropic peer workflow)
- `extra_headers` (for example a literal `x-api-key` header)
- docs, tickets, chat, or commits

Use `/providers`, `grok provider set-key … --from-env`, or a process env var
named by `env_key` on **custom** (non-product) Messages entries only.

---

## 2. Models cache and model picker

With Anthropic configured:

| Picker id | API model (curated) | Context | Max output |
|-----------|---------------------|---------|------------|
| `anthropic-claude-sonnet-5` | `claude-sonnet-5` | 1M | 128k |
| `anthropic-claude-opus-5` | `claude-opus-5` | 1M | 128k |
| `anthropic-claude-haiku-4-5` | `claude-haiku-4-5` | 200k | 64k |

Grok also refreshes the authenticated Anthropic **Models** API into
`$GROK_HOME/anthropic_models_cache.json` (owner-only). Additional
account-visible models appear as `anthropic:<upstream-id>` and are labeled
experimental (tool support is not assumed from the list alone).

Saving, testing, refreshing, or disconnecting rebuilds the authoritative
catalog and updates an already-open `/model` picker without restarting.
Connection tests use non-inference endpoints where possible so they do not
generate chat usage.

Anthropic is **never** selected as the implicit global default. New sessions
still start from the configured default (typically `grok-build`) unless you
explicitly pick an Anthropic model.

---

## 3. Native API architecture

### Repository-owned client

Direct Anthropic traffic uses a Rust client in Grok Build:

| Item | Value |
|------|--------|
| Base URL | `https://api.anthropic.com` |
| Auth header | `x-api-key` (never Bearer for direct Anthropic) |
| Version pin | `anthropic-version: 2023-06-01` |
| Request size preflight | 32 MiB max (`MAX_REQUEST_BYTES`) |
| Files beta | `files-api-2025-04-14` (only on Files **client** methods; not product UI) |

There is **no** dependency on the Claude desktop app or Claude Agent CLI for
the API peer path.

### Grok owns the agent stack

On the native Anthropic API peer path:

| Concern | Owner |
|---------|--------|
| Tool definitions and execution | Grok |
| Permissions / approval UI | Grok |
| OS sandbox | Grok |
| Host MCP servers | Grok |
| Hooks, subagents, workflows | Grok |
| Sessions, memory, compaction | Grok |
| Model HTTP (Messages) | Anthropic API via Grok client |

**Off by default** (not sent unless a future explicit product opt-in lands):

- Anthropic **server tools**
- Anthropic-hosted **MCP connector** fields
- Anthropic **server-side compaction** / container fields

Grok’s own compaction and tool loop continue to run locally.

### Supported vs deferred

| Endpoint / area | Status |
|-----------------|--------|
| `POST /v1/messages` stream | Supported (product agent path) |
| `POST /v1/messages` non-stream | Supported (product agent path) |
| `GET /v1/models`, `GET /v1/models/{id}` | Supported (catalog + cache) |
| `POST /v1/messages/count_tokens` | Supported (client library) |
| Files (`POST/GET/DELETE /v1/files`, list) | **Library only** — mock-tested client; product surface deferred |
| Batches API | Deferred |
| Admin API | Deferred |
| Managed Agents | Deferred |

Contract pins used in this tree: `anthropic-version: 2023-06-01`, Files beta
`files-api-2025-04-14`. Regenerate or extend only with matching client tests.

---

## 4. Features: structured output, thinking, tools, Files

### Structured output + tools

Native Messages can carry JSON schema output (`output_config.format`) **with**
tools present. Grok tools are **not** marked `strict: true` by default;
`supports_strict_tools` must be explicitly enabled for a model before strict
tool definitions are emitted (conservative, capped). Prefer non-strict tools
unless you have validated upstream support.

### Thinking / redacted thinking

Thinking blocks stream on a reasoning channel and are preserved for **replay**
when the model requires them. Anthropic Messages rejects thinking blocks on
subsequent turns unless thinking is properly configured — Grok’s recap /
compaction paths strip or reconfigure reasoning for backends that need it.
Redacted thinking content is never logged as plaintext secrets; treat it as
model-internal opaque data.

### Files API (client library only; product surface deferred)

The repository-owned Anthropic **client library** implements Files methods and
covers them with **mock HTTP tests**:

- upload (caller supplies in-memory bytes; the client does not walk your disk)
- list / retrieve metadata
- delete by file id (library method — **not** a Grok slash command)

There is **no** TUI flow, `grok` CLI subcommand, or agent auto-upload that
exercises Files in ordinary product use. Product integration remains
**deferred**.

Files methods add only the Files beta header when those library methods run.
Grok `/privacy` and xAI ZDR settings govern **SpaceXAI-side** retention only;
they do **not** reconfigure Anthropic retention of API traffic or any Files
objects created through the client. When a future product surface or external
caller uses the library, **explicit delete** (library `DELETE /v1/files/{id}`)
or Anthropic account controls are how objects are removed — not a Grok
user-facing “delete file” command today.

### Billing, rate limits, payload size

| Signal | Behavior |
|--------|----------|
| HTTP 429 | Backoff with rate-limit headers when present; provider-scoped identity |
| HTTP 529 | Overloaded; retry policy as for other providers |
| HTTP 401/403 | **Anthropic** identity kept; repair points to `/providers` and never starts xAI OAuth |
| HTTP 413 / >32 MiB | Preflight or fatal size error (not image-strip heuristics for pure size) |
| Rate-limit headers | Parsed for diagnostics; credentials never logged |

Safe diagnostics allowlist only: provider id/name, catalog model id, backend,
HTTP status, bounded request ids, controlled error categories. No API keys,
Authorization values, raw bodies, or prompt fragments.

---

## 5. Custom Messages gateway vs OpenRouter Claude

### Custom Messages (generic protocol)

For a non-product Anthropic-compatible Messages endpoint:

```toml
[model_providers.anthropic-gateway]
kind = "openai_compatible"   # custom Messages; not the built-in Anthropic peer
base_url = "https://messages-gateway.example/v1"
env_key = "ANTHROPIC_API_KEY"
auth_scheme = "x_api_key"
api_backend = "messages"

[model_providers.anthropic-gateway.extra_headers]
# Version pin only — never put the API key in extra_headers
anthropic-version = "2023-06-01"

[model.gateway-claude-sonnet-5]
model = "claude-sonnet-5"
model_provider = "anthropic-gateway"
name = "Claude Sonnet 5 (gateway)"
context_window = 1000000
max_completion_tokens = 128000
```

Existing hand-written Messages configs continue to work. Product Anthropic
(`kind = "anthropic"`, id `grok_build_anthropic`) is installed when you connect
via `/providers` or `set-key anthropic`; you do not need to hand-write it.

### OpenRouter Claude

```text
openrouter:anthropic/claude-…
```

These models are **OpenRouter** identity, catalog, pacing, and billing. They
are not the Anthropic peer and do not use `anthropic::api_key` unless you
separately configured OpenRouter. See [OpenRouter](11-custom-models.md) and
[docs/providers/openrouter.md](../providers/openrouter.md).

---

## 6. Claude Agent CLI (experimental subscription mode)

> **Status:** experimental. **Not** in default features. **Not** in
> `release-dist`. Ordinary release binaries do not expose the subscription CLI
> card. Full release enablement is **pending legal/product authorization**.

### Gates (all required)

| Gate | Mechanism |
|------|-----------|
| Compile feature | `claude-cli-runtime` on `xai-grok-pager-bin` / shell / pager |
| Runtime opt-in | `GROK_CLAUDE_CLI_RUNTIME=1` (also `true` / `yes` / `on`; fail-closed otherwise) |
| Binary probe | Official `claude` executable discovered and version/capability probe succeeds |

Optional path override: `GROK_CLAUDE_CLI_PATH` (absolute path to a regular
executable).

Until **all** gates pass, the catalog entry stays **hidden** and
**non-selectable**. API-key Anthropic models remain available whenever an
Anthropic key is configured, independent of the CLI feature.

### Safe-mode and auth

- Uses **safe** permission modes only — never `bypassPermissions` or
  `--dangerously-skip-permissions`.
- Relies on the **existing official Claude CLI login** already on the machine.
- Grok does **not** implement Claude login/logout, does **not** read Claude
  credential files for API keys, and does **not** inject `ANTHROPIC_API_KEY`
  into the child environment.
- Child env is scrubbed to a strict allowlist; provider and cloud secrets are
  forbidden.

### Capability / limitations matrix

| Area | Native Anthropic API peer | Claude Agent CLI (experimental) |
|------|---------------------------|----------------------------------|
| Auth | Anthropic API key (vault / env) | Official CLI subscription login only |
| Tools | Grok tool loop | Claude-owned tools; Grok displays foreign tool events |
| Permissions | Grok permission policy | Grok permission **bridge** + policy broker |
| Sandbox | Grok OS sandbox | Outer Grok process/sandbox posture; CLI child constrained |
| MCP | Grok host MCP | Strict bridge MCP config only (no arbitrary servers) |
| Compaction / memory / goals / hooks / workflows | Full Grok stack | **Not** applied inside Claude’s loop |
| Multi-turn process | Stateless HTTP turns | Session-scoped runtime reuse; persistent multi-turn child if binary advertises streaming input, else one process per turn on the retained runtime |
| Cancel | Grok turn cancel | Cancel envelope / process teardown semantics (no hang) |
| Cross-mode switch | N/A | After the first user turn, switching Native ↔ Claude CLI requires `/new` |

### Catalog rows and model selection

When the gates are open, these rows are injected (all hidden until the binary
probe succeeds):

| Catalog id | `--model` passed to the CLI | UI label |
|------------|-----------------------------|----------|
| `claude-agent-cli` | *(none — CLI keeps its own default)* | Claude Agent (CLI, Experimental) |
| `claude-agent-cli-opus` | `opus` | Claude Agent CLI · Opus (Experimental) |
| `claude-agent-cli-sonnet` | `sonnet` | Claude Agent CLI · Sonnet (Experimental) |
| `claude-agent-cli-haiku` | `haiku` | Claude Agent CLI · Haiku (Experimental) |
| `claude-agent-cli-fable` | `fable` | Claude Agent CLI · Fable (Experimental) |

Grok catalog ids are **never** forwarded as `--model`: the official CLI only
accepts its own aliases or a full model name, and rejects anything else. Unknown
ids fail closed to the CLI default.

Switching between **pinned** rows mid-session is a model change on the Claude
session and requires `/new` (same rule as Native ↔ Claude CLI). The unpinned row
imposes no model constraint, so it keeps resuming across turns whatever model the
CLI reports for the session.

### Adding rows without a Grok release

The built-in rows use the CLI's aliases, which always track the newest model in
each family. Declare `[[claude_cli.models]]` entries to pin a concrete version or
to offer a model the built-ins do not list yet:

```toml
[[claude_cli.models]]
model = "claude-opus-5"        # verbatim --model value: alias or full name
name  = "Opus 5 (pinned)"     # optional label; defaults to the model value

[[claude_cli.models]]
model = "claude-haiku-4-5"
context_window = 200000       # optional; display and accounting only
max_output_tokens = 64000     # optional
```

Those rows get id `claude-agent-cli:<model>` — the same split the Anthropic peer
uses between curated ids (`anthropic-claude-opus-5`) and upstream ones
(`anthropic:<id>`). Maximum 20 entries.

A `model` value must be non-empty, free of whitespace and control characters,
must not contain `:`, and **must not start with `-`** — that last rule keeps a
config entry from posing as a CLI flag. Invalid entries are skipped with a
warning; the valid ones around them still load.

Nothing validates a declared model against your account: there is no catalog
endpoint on this path, so a model your plan does not cover fails at turn start
with the CLI's own message ("It may not exist or you may not have access to it").

### Seeing which version an alias resolved to

An alias row asks for a family, not a version. The CLI reports the concrete model
it used for the session, and Grok keeps it in the session envelope as
`resolvedModel` — separate from the catalog id, which stays the session's
identity. `/session-info` shows both once the session has run a turn:

```
Model: Claude Agent CLI · Opus (Experimental) → claude-opus-5
```

The resolution can change between sessions when an alias moves to a newer model,
which is why it is never used as session identity and never cached.

### Foreign tools and permission bridge

Claude-owned tool calls appear in Grok’s UI as foreign tool activity. The
permission bridge is a local MCP-style broker owned by Grok:

- bridge token lives only in the bridge child environment
- never logged, never written to session NDJSON as a secret
- does not execute tools itself — only permission prompts

### Status messages (safe)

Provider status distinguishes:

- compile/runtime gates
- binary readiness / version
- subscription auth readiness (redacted label only)
- permission bridge readiness
- **explicit note** that API-key status is not applicable to the subscription path

Examples of user-facing summaries (no secrets):

- `Claude Agent CLI runtime not compiled into this build`
- `Claude Agent CLI disabled (set GROK_CLAUDE_CLI_RUNTIME=1 to opt in)`
- `Claude binary not ready: …`
- `Claude CLI binary ready; subscription not logged in`
- `Claude Agent CLI ready (subscription; no API key)`

---

## 7. Security and troubleshooting

### Credential hygiene

- Owner-only `auth.json` and model caches under `$GROK_HOME`
- Prefer full-disk encryption
- Never paste keys into prompts, tickets, or `extra_headers`
- Disconnect one provider without clearing others

### Common failures

| Symptom | What to do |
|---------|------------|
| Model missing from picker | Configure Anthropic key; run Test/Refresh in `/providers` |
| 401/403 from Anthropic | Replace key in `/providers` → Anthropic (not xAI login) |
| 429 / overloaded | Wait for backoff; check Anthropic rate limits / billing |
| Request too large | Reduce attachments/context; hard limit 32 MiB |
| Claude CLI card missing | Expected in ordinary release builds; feature + env + probe required |
| Cross-mode model switch error | Start `/new` to change Native ↔ Claude CLI |
| Want to leave Anthropic | `grok provider disconnect anthropic` |

### Logging

Set `RUST_LOG` / `GROK_LOG_FILE` for file diagnostics. Logs must never contain
API keys, bridge tokens, raw `claude auth` stdout, raw NDJSON event bodies with
secrets, or scrubbed-out argv credentials. If you see a secret in a log, treat
it as a bug and rotate the credential.

### Privacy vs Anthropic retention

Grok `/privacy`, team ZDR, and telemetry toggles apply to **SpaceXAI / Grok**
data paths. They do **not** reconfigure Anthropic’s retention of API traffic or
of any Files objects created through the client library. Grok product flows
do not auto-upload to Anthropic Files today. When the library (or a future
product surface) creates a Files object, removal is via the **client delete
API** or Anthropic account controls — not a Grok user slash command.

---

## 8. Related commands

```bash
grok provider set-key anthropic --from-env ANTHROPIC_API_KEY
grok provider disconnect anthropic
# TUI: /providers, /model, Ctrl+M (model picker from scrollback)
```

See also [Authentication](02-authentication.md), [Configuration](05-configuration.md),
[Sessions](17-sessions.md), and [Permissions](22-permissions-and-safety.md).
