# Plan: ChatGPT Subscription Configuration in the TUI

Status: ready for execution (see workflow `.grok/workflows/chatgpt-subscription-tui.rhai`)
Crate scope: `xai-grok-shell`, `xai-grok-pager`, `xai-grok-pager-bin` (compile check only)

## Context

Grok Build authenticates to the ChatGPT subscription with a Codex-compatible
OAuth login (`auth.json`, scope `openai::oauth`, separate from the OpenAI
Platform `openai::api_key` scope). Requests for `chatgpt-*` models route to
`https://chatgpt.com/backend-api/codex/responses` with the subscription token.

The subscription-side context window moved: the 5.6 family launched at 372k
server-side cap, was reduced to **272k** around July 18, 2026
(openai/codex#31860, #34619), and ~1M is an **opt-in** via client config.
Our static presets still pin 372k (5.6 family) / 400k (5.5, 5.4); the live
catalog fetch (`GET {codex-base}/models` with the OAuth token) is the primary
source and enriches the static values, but the static value is the fallback.

The TUI today manages ChatGPT purely as a **credential** (`/providers`:
login/logout/status). There is no way to see the connected account's email or
to override per-model `context_window`, and the existing
`[model.<id>]` config override is **broken for preset models**: a partial
user table replaces the preset's `ConfigModelOverride` entirely
(`install_model_presets_into` uses `or_insert_with`, providers.rs:~748), so
the entry loses `base_url`, `model` slug, and `model_provider` — the
`is_codex_base_url` route check fails and the OAuth credential is no longer
applied (401 / wrong endpoint).

## Goals

1. Per-model `context_window` override for `chatgpt-*` models, settable from
   the TUI, persisted to `~/.grok/config.toml` under `[model.<id>]`, applied
   via the existing config hot-reload (auto-compact budget + model picker).
2. `/providers` OpenAI card: show the connected ChatGPT account email and,
   when connected, a ChatGPT-subscription settings sub-view listing
   `chatgpt-*` models with their current context windows and a
   set/clear override editor (with validation).
3. Fix the preset/override merge so a partial `[model.chatgpt-*]` table
   works without breaking routing (all preset kinds, not only ChatGPT).
4. Lower the 5.6-family static fallback 372k → 272k and refresh the stale
   comment block (OpenCode parity numbers included).

## Non-goals

- No new ACP protocol methods (config file + existing hot-reload is enough).
- No changes to the OAuth flow, credential storage scopes, or wire shaping
  (`codex_wire.rs`).
- No compaction-engine changes (it already consumes the resolved
  `context_window`).
- No changes to the OpenAI Platform API-key path.
- No version bump, no `make build`/deploy.

## Phase 1 — Shell: preset/override merge + static preset refresh
Crate: `xai-grok-shell` (independent of Phases 2/3 — parallelizable)

### 1a. Merge fix (the blocker)
File: `crates/codegen/xai-grok-shell/src/agent/providers.rs`,
`install_model_presets_into` (~line 748).

Replace `config_models.entry(preset.id).or_insert_with(...)` with a merge:
when `config_models` already contains the preset key, produce a merged
`ConfigModelOverride` where **user-set fields win and the preset fills every
unset field** (`model`, `base_url`, `max_completion_tokens`, `name`,
`description`, `model_provider`, `auth_scheme`, `api_backend`,
`reasoning_effort(s)`, `supports_*`, ...). Non-`Option` fields
(`reasoning_efforts: Vec`, `extra_headers: IndexMap`): prefer the user value
when non-empty, else the preset's.

Required behavior:
- No user table → identical to today (`or_insert_with` semantics).
- Partial user table (e.g. only `context_window`) → merged entry keeps the
  preset's codex `base_url`/slug/`model_provider`; user `context_window`
  wins and flows through `ConfigModelOverride::apply` (config.rs:~4449) into
  `ModelEntry.info.context_window` → auto-compact + ACP `totalContextTokens`.
- Full user table → user fields win (today's behavior).
- Must not regress the OpenAI Platform, OpenRouter, and Anthropic presets,
  which flow through the same function.

Tests (unit, crate-local):
1. Partial user override on a chatgpt preset → merged entry keeps
   `base_url`, `model` slug, `model_provider`; user `context_window` wins.
2. No user table → resolved entry identical to pre-change behavior.
3. Full user table → user fields win.

### 1b. Static preset refresh
File: same, `static_chatgpt_oauth_presets()` (~line 1584).
- `gpt-5.6-sol` / `terra` / `luna`: `context_window` 372_000 → **272_000**
  (server-side default since ~July 18, 2026; live catalog remains primary).
- Rewrite the comment block with current facts: 372k→272k reduction
  (#31860, #34619), ~1M opt-in via client config, OpenCode parity is
  context 400k / input 272k for 5.5 and 5.6 alike.
- Keep 400_000 for `gpt-5.5`, `gpt-5.4`, `gpt-5.4-mini`.

Validation: `./grok-check.sh -p xai-grok-shell`, then
`./grok-test.sh -p xai-grok-shell` (providers/config families first, then
full crate). One helper at a time.

## Phase 2 — Pager: config-write helper
Crate: `xai-grok-pager` (independent of Phase 1 — parallelizable)

File: `crates/codegen/xai-grok-pager/src/config_toml_edit.rs`.
- Generalize `set_hint(key, value)` into
  `set_table_field(table: &str, key: &str, value: impl Into<toml_edit::Value>)`
  writing `[<table>].<key>`; keep `set_hint` as a thin wrapper (back-compat).
- Add `remove_table_key(table: &str, key: &str)` (no-op when the key is
  absent) for "clear override".
- Preserve existing guarantees: all other keys/tables stay intact, table
  created when missing, non-empty unparseable config file is never clobbered
  (error returned instead).
- Tests mirror the existing file's style: create new
  `[model."chatgpt-gpt-5.6-sol"]` with `context_window = 1000000`, update an
  existing key, preserve unrelated tables (`[hints]`), refuse malformed
  files, idempotent remove.

Validation: `./grok-check.sh -p xai-grok-pager`, then
`./grok-test.sh -p xai-grok-pager -- config_toml`, then full crate.

## Phase 3 — Pager: TUI surface
Crate: `xai-grok-pager` (+ minimal `xai-grok-shell` field if the provider
status payload needs the email). Depends on Phases 1 + 2.

Files: `views/providers_modal.rs`, `app/actions.rs` (or the command enum
file), `app/effects/mod.rs`.

1. **Account display**: when ChatGPT OAuth is connected, the OpenAI card
   detail includes the account email (e.g.
   `Connected with ChatGPT OAuth — user@example.com`). The email already
   lives in the stored OAuth tokens (`chatgpt_oauth::read_tokens`); extend
   the `ProviderStatus` payload the pager receives with the minimal field.
   Fallback: today's generic text when unavailable.
2. **Settings sub-view**: from the OpenAI row (only when connected), a
   "ChatGPT subscription" sub-view listing `chatgpt-*` catalog models with
   their current context windows; per selected model: set or clear a
   `context_window` override. Follow existing `ProviderModalMode` /
   `handle_key` / `put_line` patterns — no new UI framework.
3. **Command/effect**:
   `ProviderCommand::SetChatgptContextWindow { model_id: String,
   tokens: Option<u64> }` (`None` = clear). Effect: `spawn_blocking` config
   write via the Phase 2 helpers (set
   `[model."<model_id>"].context_window = <tokens>` or remove the key), then
   a status line noting the change takes effect via `config.toml`
   hot-reload for subsequent turns/sessions (no leader restart needed —
   the existing watcher + `x.ai/models/update` broadcast covers it).
4. **Validation**: integer in `8_000..=1_050_000`; when `> 272_000`, show a
   short note that OpenAI may apply long-context limits/pricing to the
   subscription.
5. **Tests**: unit tests for key handling; render/snapshot coverage per
   repo TUI policy.

## Phase 4 — Final validation

1. `./grok-check.sh -p xai-grok-shell` → `./grok-test.sh -p xai-grok-shell`
2. `./grok-check.sh -p xai-grok-pager` → `./grok-test.sh -p xai-grok-pager`
3. `./grok-check.sh -p xai-grok-pager-bin` (composition root still compiles)
4. `source ./grok-dev-env.sh && cargo fmt --all --check` (and clippy on the
   two touched crates if time permits)

All under the isolated `~/.grokdev` helpers, one at a time.

## Risks / notes

- **Merge semantics**: explicit-empty vs unset for `extra_headers` /
  `reasoning_efforts` — rule: non-empty user value wins, else preset.
- **Hot-reload debounce**: the watcher is async; UI copy must say the
  override applies to subsequent turns/new sessions, not instantly.
- **Email display**: read-only, sourced from the owner-only `auth.json`;
  must not leak into telemetry/debug/telemetry payloads.
- **Scope creep**: the TUI writes only the `context_window` key; no general
  model editor.
- **Cargo.lock**: source-only change — the lockfile must not change.
- Unrelated dirty files (e.g. `alibaba.txt`) must be preserved.

## Execution order

- Phases 1 and 2: **parallel** (disjoint crates, disjoint files).
- Phase 3: after both (uses the helper; needs the merge for correctness).
- Phase 4: after Phase 3.
- Review (architecture + adversarial) and repair loop between Phase 3 and
  Phase 4, per the repository's multi-agent convention.
