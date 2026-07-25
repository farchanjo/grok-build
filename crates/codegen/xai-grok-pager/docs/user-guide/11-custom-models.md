# Custom Models

Grok connects to custom model endpoints for alternative providers, self-hosted models, and overriding built-in settings. This guide explains how to select models, configure endpoints, and integrate third-party providers.

---

## Default Models

By default, Grok uses models hosted by SpaceXAI, and new sessions start with `grok-build`. Default models require no configuration. Connect xAI from `/providers` (or `grok provider connect xai`), or set an API key before sending prompts to first-party models. The TUI itself starts without a mandatory Grok login so you can also use OpenAI (ChatGPT OAuth or API key) or OpenRouter alone.

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
3. Your signed-in session token (from connecting xAI in `/providers`), for a model with no `api_key`/`env_key` of its own
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
API-key, and ChatGPT OAuth login independently. Keys are masked in the TUI and
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

> **Subagents:** when the parent session is already OpenRouter, omit `model`
> on `spawn_subagent` so the child inherits the model, fallbacks,
> `provider_preferences`, `plugins`, and key. An explicit subagent model must
> be an `openrouter:` catalog id that advertises tool support; `openai:`
> discovery slugs are still rejected for subagents. See
> [Subagents and Personas](16-subagents.md) for the full rules and per-type
> override example.

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

HTTP 401 from OpenRouter (including models such as Moonshot that are only
reached through OpenRouter) keeps **OpenRouter identity** through the shell
and pager — including auto-compaction and model-switch compaction failures.
OpenRouter and official OpenAI API-key routes are treated as provider-scoped
credentials even when the key lives in the provider vault (not inline on the
model). On 401 the runtime resolves that identity **before** any xAI session
token refresh, OIDC recovery, or WebLogin copy, so a concurrent xAI
`cached_token` / WebLogin session cannot refresh and resubmit. Per-turn
sampler reconstruction uses the same catalog-aware gate, so an xAI
`bearer_resolver` is never installed for OpenRouter/OpenAI vault models (or
for exact approved third-party hosts when the catalog misses). ChatGPT
OAuth pre-turn refresh applies only when the active model base URL is the
Codex Responses route; a disk-global ChatGPT login never overwrites
OpenRouter, OpenAI API-key, or first-party xAI credentials. The repair
prompt names OpenRouter (or OpenAI) and directs you to `/providers` to
replace or test the key. It does **not** mention `/login` and does not start
xAI OAuth. ChatGPT OAuth may remint only its own OpenAI credential. First-party
xAI session failures still use the existing session recovery path until
provider-scoped login fully replaces it.

Safe terminal diagnostics use a strict allowlist only: provider ID/name,
catalog model ID, backend, HTTP status, bounded request/generation IDs, and a
controlled error-category enum. Credentials, token/key prefixes or suffixes,
auth expiry, Authorization headers, raw provider messages, response bodies,
and prompt fragments are never logged.

#### OpenAPI baseline (developers)

Grok Build pins a compact OpenRouter OpenAPI endpoint/field inventory under
`crates/codegen/xai-grok-inference/baselines/openrouter/` with provenance
(source URL, fetch date, content SHA-256 = `90c87070…`, content size =
`1653634` bytes) and the deterministic generator
`generate_inventory.py`. The full OpenAPI document is not vendored. Tests
never fetch network content; regenerate from a **local** OpenAPI file with
the generator's exact invocation (see `PROVENANCE.md`).

Executable regression matrix (exact filters; under `~/.grokdev` env):

- Inventory integrity: `cargo test -p xai-grok-inference openrouter_baseline`
- Checklist: `cargo test -p xai-grok-inference openrouter_regression`
- Catalog/credits: `cargo test -p xai-grok-shell --lib parse_openrouter_`
- Chat/fallbacks/cost/tools/reasoning stream: `cargo test -p xai-grok-inference openrouter_`
- Mid-stream error: `cargo test -p xai-grok-inference mid_stream_error`
- 402: `cargo test -p xai-grok-inference status_402_openrouter`
- 429 pacing/retry: included in `openrouter_` (pacing + rate-limit threshold)
- Cancel: `cargo test -p xai-grok-inference cancel_in_flight`
- Moonshot OpenRouter 401 shell/pager/compaction (incl. session-method +
  WebLogin concurrent xAI session must not refresh):
  `cargo test -p xai-grok-shell --lib moonshot_openrouter_401`
  `cargo test -p xai-grok-shell --lib openai_api_key_401`
  `cargo test -p xai-grok-shell --lib first_party_xai_401`
  `cargo test -p xai-grok-shell --lib is_provider_scoped_byok_matrix`
  `cargo test -p xai-grok-shell --lib reconstruct_openrouter_vault`
  `cargo test -p xai-grok-shell --lib preturn_chatgpt_connected`
  `cargo test -p xai-grok-pager --lib openrouter_moonshot`
  `cargo test -p xai-grok-shell --lib surface_compact_auth_failure_openrouter`

#### `reasoning_effort` stripping

Some OpenRouter models hard-400 the whole request when `reasoning_effort` is
present but they never advertised reasoning support. When a model's catalog
entry disclaims support (`supported_parameters` has no `reasoning`), Grok
Build strips any `reasoning_effort` before it reaches the request body. An
explicit `Some(true)` or unknown (`None`) keeps the field, so hand-written
TOML models without a `supports_reasoning_effort` flag still honor an
explicit user-set effort.

For OpenRouter Chat Completions, Grok Build also emits the normalized
`reasoning` object (`{ "effort": "high" }`) derived from flat
`reasoning_effort`, matching the pinned OpenAPI `ChatRequest.reasoning`
field while keeping the flat field for OpenAI-compatible dual support.

OpenRouter catalog entries that advertise reasoning but omit explicit effort
options receive a standard `low` / `medium` / `high` ladder with default
`medium`. Explicit `reasoning_effort_options` and
`default_parameters.reasoning_effort` from the catalog are preserved when
present.

#### OpenRouter Responses beta (stateless)

When `api_backend = "responses"` on an OpenRouter model, Grok Build enforces
OpenRouter's stateless Responses contract:

- always sends `store = false` (even if a caller set `true`);
- strips any non-null `previous_response_id`;
- re-sends full local conversation history as `input` each turn;
- attaches the same identity-gated `models` / `provider` / `plugins`
  extensions used on Chat Completions.

Do not configure a stateful OpenRouter Responses mode; the sampler rejects
server-side response chaining for OpenRouter.

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
sort = "latency"                    # string: "price" | "throughput" | "latency"
# sort = { by = "latency" }         # object form also accepted
order = ["deepinfra/turbo"]         # preferred provider slugs, descending priority
only = []                           # provider slugs to use exclusively
ignore = []                         # provider slugs to skip
allow_fallbacks = true              # allow fallbacks when the primary fails
require_parameters = true           # only use providers supporting the request params
data_collection = "deny"            # "allow" | "deny" — training on requests
zdr = true                          # zero-data retention (opt-in)
quantizations = ["int8"]            # quantization preferences
max_price = { prompt = 0.5, completion = 2.0 }  # per-token-kind USD caps
enforce_distillable_text = true     # prefer distillable-text providers
preferred_max_latency = 250         # number or object (OpenAPI PreferredMaxLatency)
preferred_min_throughput = 100      # number or object (OpenAPI PreferredMinThroughput)

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

OpenRouter request extensions (`models` fallbacks, `provider`, `plugins`, and
the normalized Chat `reasoning` object) are gated by **provider identity**
(`kind = "openrouter"`), not by the hostname alone. Request pacing also
prefers identity; the host `openrouter.ai` remains a legacy pacing fallback.

#### `openrouter_pacing`

OpenRouter-compatible proxies that keep a non-`openrouter` identity can opt
into request spacing without claiming native OpenRouter extensions:

```toml
[model_providers.or-proxy]
kind = "custom"
base_url = "https://or-proxy.example/v1"
env_key = "OR_PROXY_API_KEY"
# Opt into OpenRouter-style request spacing (identity still Custom, so
# provider/plugins/reasoning extensions stay off):
openrouter_pacing = true

[model.proxy-model]
model = "openai/gpt-oss-120b"
model_provider = "or-proxy"

# Optional per-model override (replaces the provider-level flag):
# openrouter_pacing = false
```

Native `kind = "openrouter"` always paces regardless of this flag. Prefer
`kind = "openrouter"` with a custom `base_url` when the proxy should also emit
OpenRouter request extensions.

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

### OpenAI: ChatGPT subscription (OAuth) or API key

OpenAI is a **single provider** with two mutually exclusive credentials:

1. **ChatGPT Pro/Plus (OAuth)** — browser or device login against
   `auth.openai.com` (same public Codex/OpenCode client flow). Tokens live in
   `$GROK_HOME/auth.json` under `openai::oauth`. Requests use the ChatGPT
   Responses endpoint `https://chatgpt.com/backend-api/codex` with
   `Authorization: Bearer <access_token>` and `ChatGPT-Account-Id` when
   present. Wire headers match Codex/OpenCode (`originator=opencode`), not a
   Grok-branded client.
2. **OpenAI API key** — stored under `openai::api_key`. Requests use
   `https://api.openai.com/v1` as before.

Connecting one method clears the other. In `/providers`, select **OpenAI**,
press Enter, then choose **ChatGPT Pro/Plus (browser OAuth)** or **OpenAI API
key**. Browser PKCE is the default on macOS, Windows, and graphical Linux.
On headless Linux (no `DISPLAY`/`WAYLAND_DISPLAY`), or when
`GROK_CHATGPT_DEVICE_AUTH=1` is set, login uses the device-code path and
prints a one-time code on stderr for the OpenAI verification page.

After ChatGPT OAuth succeeds, the model picker exposes real API slugs (for
example `gpt-5.6-sol`, `gpt-5.4`) with reasoning-effort metadata. Turns run
through Grok Build's normal inference runtime and host tools — there is no
`codex app-server` process and no separate Codex agent identity.

ChatGPT Codex accepts a restricted Responses dialect (same as OpenCode /
Codex CLI). Grok Build strips Platform-only fields such as
`max_output_tokens`, `temperature`, `top_p`, and `stream_tool_calls` on that
endpoint, forces `store: false` and streaming, and sets function tools to
`strict: false`.

**Context windows (OAuth vs API key):** the OpenAI Platform API advertises
~1.05M for GPT-5.6 Sol, but the ChatGPT **subscription / Codex** product
catalog caps the same slugs lower (currently **372k** raw for GPT-5.6 Sol —
about **353k** at the usual 95% effective budget; GPT-5.5 / 5.4 family around
**400k** product context). Grok's OAuth presets use those product caps so
auto-compaction fires before the backend truncates. API-key OpenAI models
keep the larger Platform windows.

```toml
[model_providers.grok_build_openai]
kind = "openai"
# API-key mode default; OAuth overrides the base URL at runtime:
base_url = "https://api.openai.com/v1"
api_backend = "responses"

# API-key route (Platform ~1.05M). OAuth install uses product caps instead.
[model.openai-gpt-5.6-sol]
model = "gpt-5.6-sol"
model_provider = "grok_build_openai"
name = "GPT-5.6 Sol"
context_window = 372000
```

Legacy `kind = "codex"` in TOML still deserializes as OpenAI (HTTP), not as an
external agent.

Subagents use the same OpenAI models and host tool lifecycle:

```text
task(
  prompt="Review and fix the authentication module",
  description="OpenAI auth review",
  subagent_type="general-purpose",
  model="openai-gpt-5.6-sol",
  reasoning_effort="high",
  run_in_background=true
)
```

OpenAI, OpenRouter, and xAI can run concurrently as background tasks; each
uses its own HTTP credential and Grok-owned tools.

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

When you set `models_base_url`, Grok uses API key auth (`Authorization: Bearer`) instead of session auth. You do not need an interactive xAI connect — the API key is enough.

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
