# Custom Models

Grok connects to custom model endpoints for alternative providers, self-hosted models, and overriding built-in settings. This guide explains how to select models, configure endpoints, and integrate third-party providers.

---

## Default Models

By default, Grok uses models hosted by SpaceXAI, and new sessions start with `grok-build`. Default models require no configuration. Authenticate with `grok login` or an API key, then start a session.

List all available models:

```bash
grok models
```

---

## Selecting a Model

### CLI Flag

```bash
grok -p "Hello" -m grok-build
```

### Slash Command

In the TUI, switch models during a session:

```
/model grok-build
```

Or use the alias:

```
/m grok-build
```

### Model Picker (Ctrl+M)

Press `Ctrl+M` from the scrollback pane to open the model picker. It lists all available models, both built-in and custom, and lets you switch with a single keystroke. With the prompt focused, `Ctrl+M` toggles multiline input instead -- use `/model` to switch without leaving the prompt.

### Config Default

Set a persistent default in `~/.grok/config.toml`:

```toml
[models]
default = "grok-build"
```

---

## Supported API Backends

Grok supports three API backends. Set `api_backend` in your `[model.*]` config to choose which protocol the model uses:

| Value | API | Default |
|-------|-----|---------|
| `"chat_completions"` | OpenAI Chat Completions (`/v1/chat/completions`) | Yes |
| `"responses"` | OpenAI Responses (`/v1/responses`) | |
| `"messages"` | Anthropic Messages (`/v1/messages`) | |

When you omit `api_backend`, Grok uses `chat_completions`.

To send provider-specific authentication or version headers -- for example, Anthropic's `x-api-key` -- use the `extra_headers` field described below. Grok sends those headers verbatim with every request to the endpoint.

---

## Configuring Custom Models

Add custom model endpoints in `~/.grok/config.toml` under `[model.<name>]` sections:

```toml
[model.my-model]
model = "model-id"                        # Model identifier sent to the API
base_url = "https://api.example.com/v1"   # OpenAI-compatible endpoint
name = "Display Name"                     # Shown in the model picker
description = "Model description"          # Optional description
api_key = "sk-..."                        # API key for this provider (optional)
env_key = "XAI_API_KEY"                   # Env var holding the API key (optional; string or array)
api_backend = "chat_completions"          # "chat_completions", "responses", or "messages"
temperature = 0.7                         # Sampling temperature
top_p = 0.95                              # Nucleus sampling parameter
max_completion_tokens = 8192              # Maximum tokens per response
context_window = 128000                   # Total context window in tokens
extra_headers = { "x-api-key" = "sk-..." } # Extra request headers, sent verbatim (optional)
```

For several models on the same upstream, define the connection once under
`[model_providers.<name>]`, then reference it from each model with
`model_provider`. A provider can declare its identity with
`kind = "openai"`, `"openrouter"`, `"codex"`, `"xai"`, or `"custom"`.
Provider headers are inherited per key; a model can add headers or override
individual provider keys without dropping the others.

### Credential Resolution

Grok resolves the API key in this order:

1. The `api_key` field in the model config
2. The environment variable(s) named by `env_key` — a single string or an array of names. The first set, non-empty value wins (for example `env_key = ["ANTHROPIC_AUTH_TOKEN", "LC_ANTHROPIC_AUTH_TOKEN"]` for SSH `LC_*` forwarding)
3. Your signed-in session token (from `grok login`), for a model with no `api_key`/`env_key` of its own
4. The `XAI_API_KEY` environment variable (global fallback; Grok also accepts `GROK_CODE_XAI_API_KEY` for backward compatibility)

For a third-party `model_provider`, missing provider credentials fail closed:
Grok does not send an xAI session token or `XAI_API_KEY` to that provider.

### Header Privacy

First-party xAI requests carry stable session and conversation identifiers in
`x-grok-*` request headers (for example `x-grok-session-id`,
`x-grok-conv-id`, `x-grok-req-id`). These headers are gated to the xAI
provider only: OpenAI, OpenRouter, and any custom provider never receive them,
so third-party endpoints cannot correlate Grok sessions or turns. The `kind`
field on the provider (`"xai"`, `"openai"`, `"openrouter"`, `"codex"`, or
`"custom"`) drives the gate — it is identity-based, not URL-based, so a
mistyped `base_url` cannot leak the headers.

### Context Window

The `context_window` value tells Grok when to trigger auto-compaction. When you override a known model, Grok inherits that model's context window. When you define a new model and omit `context_window`, Grok defaults to 200,000 tokens, so set it explicitly to match your provider.

### Global Default Headers

To apply the same headers to *every* model in the catalog -- built-in, prefetched from `/v1/models`, or custom -- set them once under the global `[models]` section instead of repeating them per model:

```toml
[models]
extra_headers = { "X-Request-Tags" = "team=example,env=prod" }
```

These act as a base for each model's inference requests. A per-model `[model.<id>].extra_headers` entry overrides the global default **per key** (matched case-insensitively): a key set on the model wins, while any global-only keys are still inherited by that model. Like the per-model field, they ride on that model's inference calls -- not on separate services such as image generation or video generation -- which makes them handy for attribution tags (for example, cost tracking) without re-declaring them whenever a new model appears.

### Global Default Values

A few common per-model settings can also be set once under `[models]` as a default for *every* model. A per-model `[model.<id>]` value always wins; the global only fills in where a model (or the server's model list) left the field unset:

```toml
[models]
temperature                 = 0.7
top_p                       = 0.95
max_completion_tokens       = 8192
max_retries                 = 8
inference_idle_timeout_secs = 600
stream_tool_calls           = true
```

This is a small, fixed set of environment-wide knobs. Settings that identify a specific model (`model`, `base_url`, `api_key`, `context_window`, ...) cannot be defaulted this way, and a few settings with their own dedicated configuration -- auto-compaction (`[session]`), the system-prompt label (`[agent]`), and reasoning effort (`[models].default_reasoning_effort`) -- keep their existing homes.

> **Note on `stream_tool_calls`:** this one affects request *shape*, not just sampling. A few endpoints (some BYOK providers) expect it left unset; if a global `stream_tool_calls = true` causes problems for such a model, opt that model out with `stream_tool_calls = false` in its `[model.<id>]` block.

---

## Overriding Built-in Models

You can override specific fields of built-in models without redefining everything. Only specify the fields you want to change:

```toml
# Override only the API key for a default model
[model.grok-build]
api_key = "my-api-key"

# Override temperature and add a custom API key
[model.grok-build]
temperature = 0.5
api_key = "sk-custom"
```

When you override a built-in model, Grok starts with the default configuration (including the correct `base_url`), then applies only the fields you specify. Unspecified fields inherit from the default.

### Priority Order

1. Your config (`[model.*]`) -- highest priority
2. Prefetched models from remote `/v1/models`
3. Hardcoded defaults -- lowest priority

---

## Provider Examples

### Anthropic (Claude)

Use Claude models directly via the Anthropic Messages API:

```toml
[model.claude-opus]
model = "claude-opus-4-6"
base_url = "https://api.anthropic.com/v1"
name = "Claude Opus 4.6"
api_backend = "messages"
context_window = 200000
extra_headers = { "x-api-key" = "sk-ant-...", "anthropic-version" = "2023-06-01" }
```

The `messages` backend uses the Anthropic Messages protocol. Anthropic authenticates with an `x-api-key` header rather than `Authorization: Bearer`, so pass your key through `extra_headers`, which Grok sends verbatim.

### xAI, OpenAI API, and OpenRouter

Run `/providers` to manage xAI OAuth, xAI API-key, OpenAI API-key, OpenRouter
API-key, and Codex/ChatGPT login independently. Keys are masked in the TUI and
stored under separate scopes in the owner-only `auth.json`; they are never
written to `config.toml`. Removing one API key preserves every other provider
credential. For xAI, removing the API key also preserves the OAuth session.

With an OpenAI key, the picker includes the curated native Responses API
entries `openai-gpt-5.6-sol`, `openai-gpt-5.6-terra`, and
`openai-gpt-5.6-luna`. Grok Build also fetches the authenticated OpenAI
`GET /v1/models` catalog. Additional account-visible models appear as
`openai:<upstream-id>` and are explicitly labeled experimental because the
model-list response does not prove coding-agent tool support. These discovered
entries can be selected for primary conversations, but subagents reject them;
use a curated OpenAI entry for tool-using delegated work. The merged catalog is
cached in the owner-only `openai_models_cache.json`, with curated entries first.
On refresh failure, the last valid cache is used. Disconnecting OpenAI removes
that cache.

With an OpenRouter key, Grok Build fetches the authenticated
`GET /api/v1/models` catalog and exposes every returned model as
`openrouter:<upstream-id>`, for example
`openrouter:anthropic/claude-sonnet-4.6` or
`openrouter:openai/gpt-5.6-sol`. Context limits and advertised tool/reasoning
capabilities are cached in the owner-only
`openrouter_models_cache.json`. If a refresh fails, the last valid cache is
used; without a key, no OpenRouter entry is shown.

Saving, testing, refreshing, or removing a key rebuilds the authoritative
catalog and updates an already-open `/model` picker without restarting the
session. The connection test uses a non-inference endpoint, so it does not
generate model usage. Environment variables remain supported:

```bash
export OPENAI_API_KEY="..."
export OPENROUTER_API_KEY="..."
```

The following manual configuration is optional. Use it to add other models,
custom headers, endpoints, or provider-specific overrides:

```toml
[model_providers.openai]
kind = "openai"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
api_backend = "responses"

[model.openai-gpt-5-6-sol]
model = "gpt-5.6-sol"
model_provider = "openai"
name = "GPT-5.6 Sol (OpenAI API)"
context_window = 1050000
max_completion_tokens = 128000

[model_providers.openrouter]
kind = "openrouter"
base_url = "https://openrouter.ai/api/v1"
env_key = "OPENROUTER_API_KEY"
api_backend = "chat_completions"
openrouter_fallback_models = [
  "openai/gpt-5-mini",
  "google/gemini-2.5-flash",
]

[model_providers.openrouter.extra_headers]
HTTP-Referer = "https://your-project.example"
X-OpenRouter-Title = "Grok Build"

[model.openrouter-gpt-5-6-sol]
model = "openai/gpt-5.6-sol"
model_provider = "openrouter"
name = "GPT-5.6 Sol (OpenRouter)"
context_window = 1050000
max_completion_tokens = 128000
```

Both entries can coexist in the picker and in subagent model overrides. They
use the same native Grok Build agent/tool loop; only inference routing and
billing differ.

For OpenRouter, `openrouter_fallback_models` is sent through its native
`models` routing field, in the configured order, while `model` remains the
primary route. A model inherits the provider list by default and can replace
it—or disable fallback with an explicit empty list:

```toml
[model.openrouter-no-fallback]
model = "openai/gpt-oss-120b"
model_provider = "openrouter"
openrouter_fallback_models = []
```

Fallbacks may have different prices, context windows, and tool capabilities,
so choose compatible models.

#### Fallback visibility

When OpenRouter silently substitutes a fallback from `openrouter_fallback_models`,
Grok Build surfaces the served model as a non-modal scrollback note:
`served by <upstream-id> (fallback)`. The mismatch is logged for diagnosis
(models only — no request content or credentials). The served model also
appears on the `served_model` attribute of the `grok_code.api_fallback_served`
OTEL event (see [Monitoring Usage](24-monitoring-usage.md)).

#### Rate-limit handling and 429 backoff

After HTTP 429, Grok Build honors the server-reported backoff, spaces
subsequent OpenRouter requests conservatively, and shows the provider plus a
live retry countdown in the turn status. The server's `x-ratelimit-reset`
header is parsed into a seconds-until-reset window and used for both the
retry sleep and the pacer cooldown, so the recovery interval tracks the
actual limit instead of a guessed slice. When `x-ratelimit-reset` is absent,
`Retry-After` (then exponential jitter) drives the backoff.

OpenRouter gets a slightly higher 429 retry cap than the generic provider
default (2), because its per-model limits and tool-loop bursts deserve a
larger budget for the reset window to elapse. Overridable:

```bash
export GROK_OPENROUTER_RATE_LIMIT_RETRIES=3   # default
```

A non-positive or unparseable value falls back to the default so a
misconfiguration cannot silently disable all 429 retries. Normal OpenRouter
requests are spaced by two seconds by default. These process-level controls
are optional:

```bash
export GROK_OPENROUTER_MIN_REQUEST_INTERVAL_MS=2000
export GROK_OPENROUTER_RATE_LIMIT_RECOVERY_REQUESTS=8
```

Set the minimum interval to `0` to disable normal spacing. Structured logs
include safe provider, rate-limit, and generation identifiers when OpenRouter
returns them; request content and credentials are not logged.

#### Catalog TTL

The OpenRouter model catalog (`openrouter_models_cache.json`) is cached and
revalidated in the background after a 6-hour freshness window (stale-while-
revalidate): the picker and session keep using the last-good models while a
refresh runs. If a refresh fails, the last valid cache is used; without a
key, no OpenRouter entry is shown. Override the window:

```bash
export GROK_OPENROUTER_CATALOG_TTL_SECS=21600   # default 6h
```

Set it to `0` to disable automatic background revalidation (the cache is
refreshed only on explicit `/providers` test/save/refresh actions).

#### Provider-aware errors

API error messages name the upstream that failed rather than hardcoding
"Grok": when OpenRouter diagnostics carry the selected upstream's
`provider_name`, that name appears in the error copy. HTTP 402 (Payment
Required) — OpenRouter's out-of-credits signal — produces a dedicated,
actionable message (`<provider> account out of credits — add credits to
continue.`) and is never retried (billing failure is deterministic). Long
structured error bodies are truncated head+tail (120 + 140 chars with an
ellipsis) so the actionable portion OpenRouter puts at the end is never lost;
non-JSON bodies (HTML edge pages) are never surfaced verbatim.

#### `reasoning_effort` stripping

Some OpenRouter models hard-400 the whole request when `reasoning_effort` is
present but they never advertised reasoning support. When a model's catalog
entry disclaims support (`supported_parameters` has no `reasoning`), Grok
Build strips any `reasoning_effort` before it reaches the request body. An
explicit `Some(true)` or unknown (`None`) keeps the field, so hand-written
TOML models without a `supports_reasoning_effort` flag still honor an
explicit user-set effort.

#### `provider_preferences`

OpenRouter's native `provider` request-body field lets you control routing,
privacy, and price. Declare it on the provider block and/or per model. A
model-level `[model.<id>.provider_preferences]` table **replaces** the
provider-level object entirely for that model (fields not set on the model
are not inherited from the provider level). Only emitted for
`kind = "openrouter"`; an all-empty object omits the `provider` key from the
wire entirely.

```toml
[model_providers.openrouter]
kind = "openrouter"
base_url = "https://openrouter.ai/api/v1"
env_key = "OPENROUTER_API_KEY"
api_backend = "chat_completions"

[model_providers.openrouter.provider_preferences]
sort = "latency"                    # "price" | "throughput" | "latency"
order = ["deepinfra/turbo"]         # preferred provider slugs, descending priority
only = []                           # provider slugs to use exclusively
ignore = []                         # provider slugs to skip
allow_fallbacks = true              # allow fallbacks when the primary fails
require_parameters = true           # only use providers supporting the request params
data_collection = "deny"            # "allow" | "deny" — training on requests
zdr = true                          # zero-data retention (opt-in)
quantizations = ["int8"]            # quantization preferences
max_price = { prompt = 0.5, completion = 2.0 }  # per-token-kind USD caps

# A model-level table replaces the provider-level object for this model:
[model.my-model.provider_preferences]
sort = "throughput"
data_collection = "deny"
```

Built-in defaults: when you connect OpenRouter through `/providers`, Grok
Build applies `data_collection = "deny"` and `require_parameters = true` by
default, and sets `X-OpenRouter-Title = "Grok Build"`. These privacy defaults
are not applied to a hand-written `[model_providers.openrouter]` block — set
them explicitly there.

`zdr` is opt-in. Zero-data retention narrows the pool of compliant providers,
so enabling it may reduce routing options or raise cost. Set it only when
your compliance posture requires it.

#### `plugins`

OpenRouter's native `plugins` request-body array enables server-side
post-processing. Declare it on the provider block and/or per model; a
model-level list replaces the provider-level list for that model. Each entry
is a table with a required `id` and arbitrary provider-specific knobs that
serialize inline (flattened, not nested under an `extra` key). Only emitted
for `kind = "openrouter"`; an empty list omits the `plugins` key entirely.

```toml
[model_providers.openrouter]
kind = "openrouter"
base_url = "https://openrouter.ai/api/v1"
env_key = "OPENROUTER_API_KEY"
plugins = [
  { id = "response-healing" },
  { id = "web", max_results = 3 },
]

# A model-level list replaces the provider-level list for this model:
[model.my-model]
model = "openai/gpt-5.6-sol"
model_provider = "openrouter"
plugins = [{ id = "response-healing" }]
```

#### Credits display

`/providers` shows the OpenRouter account's remaining credits (from
`GET /api/v1/key`). When the remaining balance falls below the low-credit
threshold, the status flags it with a ⚠ low-balance marker. Override the
threshold:

```bash
export GROK_OPENROUTER_LOW_CREDIT_USD=1.0   # default $1
```

The exact balance never leaves the process except as a bucket label on the
`grok_code.openrouter_credits` OTEL event (see
[Monitoring Usage](24-monitoring-usage.md)).

### Codex with a ChatGPT subscription

ChatGPT subscription access is not a general OpenAI API credential. Install
the official Codex CLI, then select **Codex / ChatGPT** in `/providers` and
press Enter to start the official browser login. Grok Build never asks for or
stores the account password or ChatGPT token. The same login can be managed
outside the TUI:

```bash
codex login
codex login status
# On a remote/headless machine:
codex login --device-auth
```

Codex CLI version 0.145.0 or newer is required. After a successful login, Grok Build asks the native app-server for its
paginated `model/list` catalog. The server's default model is exposed through
the stable `codex-subscription` alias; every other visible model is selectable
as `codex:<model>`, for example `codex:gpt-5.6-terra` or
`codex:gpt-5.6-luna`. The choices disappear after logout or an invalid login.
Use `/model` to select any of them without an OpenAI API key. The following
manual form is only needed to override the command or define a pinned entry:

```toml
[model_providers.codex]
kind = "codex"
# Optional override; this is the default:
command = ["codex", "app-server", "--stdio"]

[model.codex-subscription]
model = "gpt-5.6-sol"
model_provider = "codex"
name = "Codex (ChatGPT subscription)"
context_window = 1050000
```

Codex is a complete coding agent rather than a sampling endpoint. Primary
turns and subagent turns therefore run through the official native app-server,
not through the OpenAI API or Grok Build's inference sampler. During primary and subagent Codex turns, Grok Build forwards app-server stream
notifications into the same ACP surfaces used by native models: assistant text
deltas, reasoning/thought chunks, tool call cards (commands, file edits, MCP,
host tools), plan updates, and completion status. Codex `userMessage` items are
never rendered as fake tool cards: on the primary surface the host already owns
the user bubble, and on a subagent surface the text appears as a normal user
chunk. Only an explicit allowlist of tool-like item kinds increments tool
statistics. Terminal metadata (model, provider, token usage, tool-call counts)
is always persisted, including when the body already streamed live.

Cancellation (Esc / Ctrl+C) sends a graceful `turn/interrupt` to the app-server
before the process is reaped. Interjections, skill reminders, monitor events,
and the first-turn memory reminder are drained into the Codex prompt at safe
points (before the turn and after completion); mid-turn steering uses
interrupt-and-reprompt — Codex owns its in-flight loop. Stream channels are
bounded with backpressure: consecutive text/reasoning deltas may coalesce when
the UI is slow, but lifecycle and tool events are never dropped. Host dynamic
tools (`task`, …) run off the stdout drain so a slow tool cannot stall protocol
reads.

Subagent streams land on the child session view (`codex:<subagent-id>`) so
opening a Codex task shows the full native turn live. The child also writes a
durable transcript (`updates.jsonl`) and fsyncs it before completion is
announced; file edits are folded into the parent's "files touched" set. The
primary session persists its private Codex thread link in `codex_thread.json`
(async atomic write) and resumes it on subsequent Codex turns.

**Rewind limitation:** Codex applies file edits inside its own tool loop, so
Grok's pre-edit snapshot tracker may not capture those changes. Rewind on
Codex-made edits is best-effort and may fail safe rather than restore a full
pre-edit snapshot. Context-usage gauges on Codex sessions reflect the usage
reported by app-server at turn end (not a live estimate of the Codex-owned
thread).

API credential fields on a Codex provider are ignored and accidental direct
inference fails closed:

```text
task(
  prompt="Review and fix the authentication module",
  description="Codex auth review",
  subagent_type="general-purpose",
  model="codex-subscription",
  reasoning_effort="high",
  run_in_background=true
)
```

Codex tasks use the official JSONL app-server protocol over stdio, inherit the
local `codex login` session, run non-interactively with approval escalation
disabled, and can execute concurrently with OpenAI/OpenRouter subagents.
`resume_from` accepts the original Grok Build subagent ID. The runtime resolves
the private Codex thread ID from session metadata, validates session ownership,
provider, model, working directory, and sandbox, then continues it through
`thread/resume`.

For a primary Codex conversation, Grok Build exposes its own `task`,
`get_task_output`, and `kill_task` lifecycle tools to the app-server as dynamic
tools. Native Codex multi-agent is disabled in this route so Grok Build remains
the single owner of depth limits, permissions, provider selection, task state,
and metrics. Codex subagents retain their resolved role, persona, memory, cwd,
and sandbox ceiling, but cannot create another level of children.

To use OpenAI, OpenRouter, and Codex concurrently, launch each task in the
background:

```text
task(prompt="Analyze the API", description="OpenAI analysis",
     model="openai-gpt-5.6-sol", run_in_background=true)
task(prompt="Review the implementation", description="OpenRouter review",
     model="openrouter:anthropic/claude-sonnet-4.6", run_in_background=true)
task(prompt="Implement the selected fix", description="Codex implementation",
     model="codex:gpt-5.6-terra", run_in_background=true)
```

Each call returns its own subagent ID immediately. OpenAI and OpenRouter use
their native HTTP APIs; Codex uses its native app-server protocol. No ACP
adapter is used as a provider transport. xAI remains available to the primary
session and to any subagent that inherits or explicitly selects an xAI model.

All provider routes receive the same neutral software-engineering and
architecture role. A backend is not told to identify as Grok merely because
the host executable or repository uses that name. When asked about its
underlying model/provider, it reports only explicit runtime metadata.

### Ollama (Local Models)

Run models locally with [Ollama](https://ollama.ai):

```toml
[model.ollama-codellama]
model = "codellama"
base_url = "http://localhost:11434/v1"
name = "CodeLlama (Ollama)"
```

Make sure Ollama is running (`ollama serve`) and the model is pulled (`ollama pull codellama`).

### Together AI

```toml
[model.together-mixtral]
model = "mistralai/Mixtral-8x7B-Instruct-v0.1"
base_url = "https://api.together.xyz/v1"
name = "Mixtral 8x7B"
env_key = "TOGETHER_API_KEY"
```

### Local OpenAI-Compatible Server

Any server that implements the OpenAI Chat Completions or Responses API:

```toml
[model.local-llama]
model = "llama-3.1-70b"
base_url = "http://localhost:8080/v1"
name = "Local Llama"
temperature = 0.8
```

---

## Custom Models Endpoint

Point Grok at a custom OpenAI-compatible `/v1/models` endpoint instead of the default. Use this when your models sit behind a corporate gateway or a self-hosted inference service.

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `GROK_MODELS_BASE_URL` | Yes | Base URL for inference. Grok fetches the model list from `{base_url}/models`. |
| `XAI_API_KEY` | Yes | API key sent as `Authorization: Bearer`. Grok also accepts `GROK_CODE_XAI_API_KEY`. |
| `GROK_MODELS_LIST_URL` | No | Override the model-list URL when it differs from `{base_url}/models`. |

### Setup

```bash
export GROK_MODELS_BASE_URL="https://api.acme.com/v1"
export XAI_API_KEY="xai-..."
grok
```

### Config File Alternative

```toml
[endpoints]
models_base_url = "https://api.acme.com/v1"

# Override only the API key for a specific model
[model.grok-build]
api_key = "my-api-key"
```

When you use `[endpoints]` with partial model overrides, Grok inherits the `base_url` from the endpoints config, so you do not need to specify it in each `[model.*]` section.

### Auth Behavior

When you set `models_base_url`, Grok uses API key auth (`Authorization: Bearer`) instead of session auth. You do not need `grok login` -- the API key is enough.

---

## Web Search Model

The `web_search` tool uses a separate model. Configure it with:

```toml
[models]
web_search = "grok-4.20-multi-agent"
```

Or via environment variable:

```bash
export GROK_WEB_SEARCH_MODEL="grok-4.20-multi-agent"
```

If you point web search at a custom model, you also need a `[model.*]` entry so Grok can reach it. Server-side ("backend") web search runs only when the model sets `supports_backend_search = true` (and the build enables backend search); it does not depend on `api_backend`:

```toml
[models]
web_search = "my-custom-model"

[model.my-custom-model]
model = "my-custom-model"
supports_backend_search = true
```

---

## Using Custom Models

```bash
# List available models (including custom)
grok models

# Use in the TUI via slash command
/model my-model

# Use in headless mode
grok -p "Hello" -m my-model

# Set as default in config.toml:
[models]
default = "my-model"
```

---

## Enterprise Deployment

A complete config for an enterprise deployment with custom models:

```toml
[cli]
auto_update = false

[auth]
auth_provider_command = "/usr/local/bin/my-company-auth-provider"
auth_provider_label = "Acme Corp"
auth_token_ttl = 3600

[models]
default = "company-grok"

[model.company-grok]
model = "grok-build"
base_url = "https://grok-proxy.acme.com/"
name = "Grok Build Latest (Proxy)"
context_window = 128000

[features]
telemetry = false
```

---

## Troubleshooting

### Model Not Found

```bash
# List available models
grok models

# Check config.toml for typos in [model.*] sections
```

### Connection Errors

Verify the endpoint is reachable:

```bash
curl -s https://api.example.com/v1/models \
  -H "Authorization: Bearer $XAI_API_KEY"
```

### Debug Logging

```bash
RUST_LOG=debug GROK_LOG_FILE=/tmp/grok.log grok
tail -f /tmp/grok.log
```

Look for log entries containing `model` or `sampling` to trace model selection and API calls.
