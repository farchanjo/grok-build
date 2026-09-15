# Running This Fork With Zero xAI Dependency

A practical reference for running this fork against a third-party or local
OpenAI-compatible gateway, with every xAI-hosted surface switched off.

Scope and status:

- The work lands in two commits on `fcustom`: `ab617625` (`feat(xai-free): run
  without an xAI account, by default`) and `a8c4d2ea` (`feat(xai-free): one
  switch for xAI surfaces, identity from the endpoint`). Three follow-ups are
  committed on top of them: `dc22e438` (config tier for the switch, early
  failure with no credential), `44d7a2a4` (a custom models endpoint survives
  `remote_fetch=0`) and `bcd261e3` (a headless turn completes without a grok.com
  session). The other five commits in the batch are supporting changes, not part
  of the off-xAI surface itself (see [What is verified](#4-what-is-verified)).
- All `file:line` references are from the working tree at the time of writing,
  against committed code (HEAD `a68c2b6a`). A concurrent session still edits the
  same four files — `xai-grok-shell-base/src/util/mod.rs`,
  `xai-grok-shell/src/agent/app.rs`, `xai-grok-shell/src/agent/config.rs`,
  `xai-grok-shell/src/agent/init.rs` — so treat their line numbers as
  approximate. See [Follow-ups](#follow-ups-committed).
- Everything below was cross-checked against the code. Where the evidence is a
  commit message rather than a re-measurement, it says so.

## 1. The switches

### Environment variables

| Variable | Type | Default | Precedence | Implemented at |
| --- | --- | --- | --- | --- |
| `GROK_XAI_ENABLED` | strict bool | `true` | env > `[xai] enabled` config key | const `xai-grok-shell-base/src/util/mod.rs:93`; resolved `:109`/`:119`; applied `xai-grok-shell/src/agent/init.rs:42` (top of `bootstrap`, before `resolve_config`) and `:174`; consumed `xai-grok-shell/src/auth/model.rs:148-151` |
| `GROK_REMOTE_FETCH` | strict bool | falls through to config layers; final default `true` | env > requirements (MDM > system > user) > managed > system managed > user `config.toml` | const `xai-grok-shell/src/util/config/resolve/features.rs:5`; resolve `:44`; env tier `:69` |
| `GROK_API_KEY` | bearer string | unset | first of three, then `XAI_API_KEY`, then legacy `GROK_CODE_XAI_API_KEY` | `xai-grok-shell/src/agent/auth_method.rs:33`, read order `:44-47` |
| `GROK_MODELS_BASE_URL` | URL | unset → inference goes to the proxy | env seeds the default; an explicit `[endpoints]` key wins | `xai-grok-shell/src/agent/config.rs:569`; `resolve_inference_base_url` `:320` |
| `GROK_MODELS_LIST_URL` | URL | `{models_base_url}/models` → `{proxy}/models` | env seeds the default; an explicit `[endpoints]` key wins | `xai-grok-shell/src/agent/config.rs:570`; `resolve_models_list_url` `:551-560` |
| `GROK_SETTINGS_FETCH_TIMEOUT_SECS` | integer seconds | `10` (`0`/blank/junk keeps the default) | env | const `xai-grok-shell/src/remote/client.rs:18`; parser `:644`; used by `fetch_settings_blocking` `:589` |
| `GROK_CHANGELOG_OFFLINE` | presence flag | unset → fetch CDN | any non-empty value except `0` skips the CDN | `xai-grok-shell-base/src/util/changelog.rs:204`, used `:103` |
| `GROK_DISABLE_AUTOUPDATER` | truthy flag | unset → check for updates | env | `xai-grok-pager-bin/src/main.rs:2660` |
| `GROK_TELEMETRY_ENABLED` | bool or mode | falls through to `[features] telemetry` | env > config | `xai-grok-shell/src/agent/config.rs:2615` |
| `GROK_FEEDBACK_ENABLED` | bool | falls through to `[features] feedback` | env > requirements > config > remote settings | `xai-grok-shell/src/agent/config.rs:2739` |
| `GROK_TELEMETRY_TRACE_UPLOAD` | bool | inherits the telemetry toggle when unset | env > `[telemetry] trace_upload` | `xai-grok-telemetry/src/config.rs:172` |

"Strict bool" means the parser (`xai_grok_config::env_bool`) accepts
`1/true/yes/on/enabled` and `0/false/no/off/disabled`; anything else —
including a blank value or a typo — falls through to the next tier instead of
silently flipping the switch. `GROK_REMOTE_FETCH` and `GROK_XAI_ENABLED` are
deliberate about this.

`GROK_XAI_ENABLED` has exactly one production consumer:
`GrokAuth::is_xai_auth` returns `false` while it is off
(`xai-grok-shell/src/auth/model.rs:148-151`). That single choke point is what
quiets share links, billing, cloud sandboxes, Writeback, remote
sessions/workspaces, managed MCP, trace upload and telemetry identity even
when an xAI credential is present.

The relay is picked *before* `bootstrap`, so the switch is applied at the top of
the three agent entry points as well (`agent/app.rs`: `run_leader`,
`run_headless_inner`, `run_stdio_agent` → `agent/init.rs::apply_xai_switch`,
same place `bootstrap` applies it, before `resolve_config`). Without that
hoisting a run holding a grok.com session would still open the relay websocket
with the switch off.

The `[endpoints]` table is a serde-defaulted field
(`xai-grok-shell/src/agent/config.rs:1649`), so an explicit key there beats the
env var of the same name; the env var is what applies when the table leaves
the key out.

### Config keys

| Key | Default | Notes | Implemented at |
| --- | --- | --- | --- |
| `[xai] enabled` | `true` | Config twin of `GROK_XAI_ENABLED`; the strict env tier wins over it and a blank/junk env value falls through to it. Reaches the same process-wide flag, so it rides the normal config merge (user, managed, requirements). | `xai-grok-shell/src/agent/config.rs:1258-1273`; resolve `xai-grok-shell-base/src/util/mod.rs:119`; applied `xai-grok-shell/src/agent/init.rs:42`, `:158` |
| `[features] remote_fetch` | `true` | Config twin of `GROK_REMOTE_FETCH`; the env tier wins. A managed or requirements pin beats a stray user `remote_fetch = true` on purpose. | `xai-grok-shell/src/agent/config.rs:647`; requirement pin `xai-grok-shell/src/config/mod.rs:1307` |
| `[features] telemetry` | product default | Master switch for product analytics. | `xai-grok-shell/src/agent/config.rs:629` |
| `[features] feedback` | product default | Feedback popups and `/feedback`. | `xai-grok-shell/src/agent/config.rs:631` |
| `[telemetry] trace_upload` | follows telemetry | Session trace upload. | resolved at `xai-grok-shell/src/agent/config.rs:2664` |

There is deliberately **no remote-settings tier** for `remote_fetch`: remote
settings are exactly what is unreachable when the knob is needed
(`features.rs:40-43`).

## 2. The recipe

A fully off-xAI run, no config file edits:

```sh
# 1. A plain bearer for your own gateway.
export GROK_API_KEY='<bearer for your gateway>'
export GROK_MODELS_BASE_URL='https://gateway.example/v1'

# 2. The two kill switches.
export GROK_XAI_ENABLED=0
export GROK_REMOTE_FETCH=0

# 3. Optional: silence the remaining xAI-hosted side channels.
export GROK_CHANGELOG_OFFLINE=1
export GROK_DISABLE_AUTOUPDATER=1
export GROK_TELEMETRY_ENABLED=false
export GROK_FEEDBACK_ENABLED=false
```

What each line buys:

| Line | Effect | Evidence |
| --- | --- | --- |
| `GROK_API_KEY` | Makes the bearer the first-checked credential, so the run does not need `auth.json` or a grok.com login. The value is sent verbatim as `Authorization: Bearer`. | `auth_method.rs:33`, `:44-47` |
| `GROK_MODELS_BASE_URL` | Turns the endpoint into a "custom endpoint": inference goes there, the model list is read from `{base}/models`, and the bundled xAI catalog is skipped. It also stops the api-key route from pointing at `api.x.ai`. | `config.rs:569`, `:282`, `:320`, `:331`, `:3859-3868` |
| `GROK_XAI_ENABLED=0` | Turns every xAI-hosted surface off process-wide, credential or not — except the relay gate, which runs before the switch (section 1). | `util/mod.rs:93`, `:119`, `auth/model.rs:148-151` |
| `GROK_REMOTE_FETCH=0` | Disarms the startup model-catalog fetch and the `/v1/settings` fetch (a custom `GROK_MODELS_BASE_URL` catalog still loads). Startup does not wait on `cli-chat-proxy.grok.com` for either — unless a deployment key or signed-in team makes the same prefetch thread sync managed config, which has its own `[features] managed_config` gate. | `features.rs:44`, callers `agent/mvp_agent/agent_ops.rs:860`, `:966`; pair gate `agent/models.rs:1938-1951`; managed sync `agent/models.rs:2114-2124`, `managed_config.rs:469` |
| `GROK_CHANGELOG_OFFLINE=1` | Skips the `https://x.ai/cli/changelogs` CDN and reads the local cache only. | `changelog.rs:15`, `:103`, `:204` |
| `GROK_DISABLE_AUTOUPDATER=1` | No update check. `--no-auto-update` does the same thing per-run. | `main.rs:2660` |
| telemetry / feedback off | No product-analytics events and no feedback endpoint calls. | `config.rs:2615`, `:2739` |

The first four lines are the whole off-xAI configuration; the remaining lines
only remove side channels that would otherwise still be attempted.

## 3. What still touches xAI

### Hard (the run breaks without the other side)

| Case | What happens | Where |
| --- | --- | --- |
| No credential, no custom endpoint, model still bound to xAI | Fails before the first request, with the remedy in the message (quoted below) and exit 1 on `grok -p`. Nothing synthesizes a local endpoint. | preflight `agent/mvp_agent/acp_agent.rs:790-793`; message `agent/config.rs:5948`; exit `xai-grok-pager-bin/src/main.rs:2190-2194`, `xai-grok-pager/src/headless.rs:1049-1053`; warning path `agent/config.rs:5853-5865` |
| `grok agent headless` | A grok.com session gates only the relay, and the relay is session-authenticated (`is_xai_auth`). With no session the run does not bail: the agent is served over stdio (ACP) instead, after `No xAI session: serving the agent over stdio (ACP); the grok.com relay is off.` on stderr. With a session present, `GROK_XAI_ENABLED=0` does *not* remove the relay on this path (section 1). | gate `agent/relay.rs:71`; fallback `agent/app.rs:620-627` |

The no-credential message is verbatim
(`agent/config.rs:5948`), with the effective model id interpolated:

```text
No credential for model `<model>`: set GROK_API_KEY (a plain bearer) together with GROK_MODELS_BASE_URL=<endpoint>/v1 for a non-xAI gateway, or sign in with `grok provider connect xai`
```

`grok -p` (single-turn, pager-side) is *not* affected: it never used the
relay. That is the path the measurements below exercise.

### Soft (warns or degrades)

| Surface | Behavior when xAI is off | Where |
| --- | --- | --- |
| Startup settings fetch | Advisory only. 10 s per attempt, 2 attempts, then a process-wide latch so later callers (prefetch thread, shell fallback, post-auth refresh) return immediately. | `remote/client.rs:589`, `:18`, `:20`; callers `agent/mvp_agent/agent_ops.rs:860`, `:966`, `agent/subscription_check.rs:140` |
| Bundled model catalog | Used only when no custom endpoint is configured; with `GROK_MODELS_BASE_URL` set it is skipped. | `agent/config.rs:3859-3868`, `:4144` |
| Imagine / video | `image_gen`, `image_to_video`, `reference_to_video` call the Imagine API directly; a third-party gateway does not serve them. | `xai-grok-tools/.../image_gen/mod.rs:31` |
| Voice dictation (STT) | Pager-owned direct-to-`api.x.ai` client. | `xai-grok-pager/src/client_identity.rs:6` |
| Share links | `/share` requires xAI auth and builds a grok.com URL. | `extensions/share.rs:35`, `extensions/auth_gate.rs:6` |
| Billing / usage | Both handlers require xAI auth. | `extensions/billing.rs:201`, `:292` |
| Cloud sandboxes | `x.ai/cloud/*` extension handlers require xAI auth. | `agent/mvp_agent/acp_agent.rs:3837-3960` |
| Managed MCP | Managed server list comes from the backend. | `xai-grok-config-types/src/lib.rs:679` |
| Managed config sync | Separate `[features] managed_config` gate (the prefetch thread's deployment sync rides it too); `managed_policy_gate` still fails closed at startup. | `managed_config.rs:469`, `agent/models.rs:2114-2124`, `agent/init.rs:44` |
| Trace upload | Off via `[telemetry] trace_upload` / `GROK_TELEMETRY_TRACE_UPLOAD`; uploads ride first-party auth. | `xai-grok-telemetry/src/config.rs:172`; the gate is asserted in `xai-grok-pager/tests/pty_e2e/storage_upload_parks_on_401_and_drains_after_recovery.rs:19` |

There is no local equivalent for share links, billing, cloud sandboxes, the
managed MCP gateway, managed config, or Imagine/video/STT.

## 4. What is verified

Every number below is **quoted from a commit message** on `fcustom`. They were
not re-measured for this document. All runs used `grok -p "Reply with exactly:
OK"` on a debug binary, with xAI blackholed via `10.255.255.1` (connect
timeouts, not RST); the `8f6e0b09` measurement additionally states an isolated
profile.

From `ab617625`:

| Scenario | Before | After |
| --- | --- | --- |
| `GROK_API_KEY` + `GROK_MODELS_BASE_URL`, no `auth.json`, xAI blackholed | n/a | exit 0, 9.2 s, zero `api.x.ai`, zero settings fetch |
| xAI credential present + `GROK_REMOTE_FETCH=1`, xAI blackholed | timeout > 120 s | exit 0, 47 s |

Same commit: the startup settings fetch budget went from the shared client's
`30 s x 3 = 91 s` to `10 s x 2` plus a process-wide terminal-failure latch.

From `a8c4d2ea`:

- `GROK_XAI_ENABLED=0 GROK_REMOTE_FETCH=0`, xAI blackholed, xAI credential
  present: exit 0 in **14.6 s**, zero settings fetch, and the
  "xAI surfaces disabled" line emitted. The commit states that together with
  Phase 1 this is the whole off-xAI configuration, in two env vars.

From the supporting commits:

- `8f6e0b09` (startup overlap, MCP compat): on a trivial `-p` prompt, median of
  3, debug binary — time to prompt sent 3.91 s → 3.00 s, total 9.70 s →
  7.12 s; the MCP server list drops from 6 to 2 and the always-failing
  `merkle` server (sourced from `~/.claude.json`) is gone; the
  `remote_settings fetched as shell-level fallback` log line disappears.
- `d1ff8377` (bounded first-prompt MCP wait): against a deliberately slow MCP
  server, with the budget the wait logs `outcome="timed_out" elapsed_ms=1002`
  and the prompt proceeds; with `0` it waits `elapsed_ms=9750`. Budget
  default 5 s, overridable via `GROK_MCP_PROMPT_WAIT_MS` >
  `[mcp].prompt_wait_ms`; `0` restores the unbounded historical wait. The TUI
  is untouched.
- `be831567` (build-script watch paths): after the fix, two consecutive
  `cargo check -p xai-grok-tools-api` runs recompiled 0 units (previously
  every run recompiled 25); `VERSION_WITH_COMMIT` stayed byte-identical
  (`1.4.8 (c356b0eb)`).
- `cc2f5443` (test flake): 4/4 failures before, 5/5 in isolation and 46/46 for
  the whole `team_managed_config` binary after; `-j 2` also passed before the
  fix, which pinned it on starvation rather than source.
- `9fa2e3dd` (dev-env under zsh): `source ./grok-dev-env.sh` used to abort
  before exporting anything, silently building into `target/` instead of
  `target-dev` (11 GB of stray artifacts in one incident).

Test families named by the commits: `settings_fetch_budget`, `remote_fetch`,
`default_models`, pager `headless`, `is_xai_auth` (8), `provider_identity`
(2), `billing` (9). One known pre-existing failure is named in `a8c4d2ea`:
`share`-filtered `sibling_accounts_do_not_share_credentials`.

## 5. What is not solved

- **The raw no-credential case.** With no key, no custom endpoint and a stale
  xAI model cache, the run no longer reaches the server: the authenticate
  preflight fails fast with the remedy in the message and a non-zero exit (see
  the hard table in section 3). Nothing synthesizes a local endpoint, and an
  unresolvable current model still reports nothing.
- **The relay.** The relay needs a grok.com session; without one the agent is
  served over stdio instead. `GROK_XAI_ENABLED=0` does *not* remove the relay
  on the leader and headless paths, because the relay is decided before the
  switch is applied (section 1).
- **Products with no local equivalent.** Share links, billing/usage, cloud
  sandboxes, managed MCP, managed config, Imagine/video/STT.
- **Model identity nuance.** `provider_identity_for_model` now derives the
  identity from the base URL when `model_provider` is absent, but **loopback
  stays first-party** (`http://localhost:8000/v1` is treated as xAI, so
  `x-grok-*` headers are still sent). LAN and third-party hosts become
  `Custom`. `a8c4d2ea` documents loopback as intentional.

### Known sharp edges

- `GROK_API_KEY` is written to `auth.json` (`xai::api_key`) by the authenticate
  step (`agent/mvp_agent/acp_agent.rs:759-767`) and the stored copy stays live:
  it is injected into `XAI_API_KEY` at startup when no env var is set
  (`acp_agent.rs:396-402`), and the provider vault returns it ahead of the env
  var (`agent/providers.rs:1672-1689`). Rotating the key means clearing that
  entry, not just re-exporting the variable.
- `grok agent headless` exits on stdin EOF when it serves over stdio: the stdin
  bridge shuts the ACP input down (`agent/app.rs:389-404`), so a harness that
  closes stdin ends the process.
- The TUI does not surface the new no-credential message — the pager catches
  `authenticate` errors and keeps the login-aware welcome flow
  (`xai-grok-pager/src/acp/mod.rs:740-752`). `grok -p` does surface it.
- Loopback is first-party (`is_cli_chat_proxy_url`,
  `xai-grok-shell-base/src/util/mod.rs:60-70`), so `http://localhost:8000/v1`
  still gets `x-grok-*` headers — see the identity note above.

### Follow-ups (committed)

Two workstreams this document previously described as in flight are committed;
the references above reflect them:

1. **`[xai] enabled` config twin.** `xai-grok-shell-base/src/util/mod.rs:119`
   gains `resolve_xai_enabled_from(config_value)`, `agent/config.rs:1258-1273`
   gains `XaiConfig`, and `agent/init.rs:42` applies `cfg.xai.enabled`. Env
   still wins, and a blank/junk env value falls through to the config tier.
2. **`grok agent headless` without a grok.com session.** `agent/app.rs:620-627`
   makes the session optional: a run holding only a bearer env var proceeds
   session-less, and when the relay cannot be built the agent is served over
   stdio ACP with
   `No xAI session: serving the agent over stdio (ACP); the grok.com relay is off.`
   The headless row in section 3 is a fallback now, not a hard failure.

`git status` at the time of writing: the four files above still carry
concurrent-session edits, plus untracked scratch files (`alibaba.txt`,
`codesign0..2`, `hello-worker/`) that are unrelated.

## 6. How to verify locally

### Blackhole technique

`10.255.255.1:9` is a non-routable address with no listener: a TCP connect
there hangs until the client timeout instead of being refused with an RST.
That is what makes it a useful xAI stand-in — it exercises the timeout budget
rather than a fast failure.

Point both xAI hosts at it through their env overrides
(`agent/config.rs:564-566`) rather than editing `/etc/hosts` (an `[endpoints]`
key in `config.toml` would win over the env var, so check both if the run
still reaches xAI):

```sh
export GROK_CLI_CHAT_PROXY_BASE_URL='http://10.255.255.1:9/v1'
export GROK_XAI_API_BASE_URL='http://10.255.255.1:9/v1'
```

Then run the off-xAI recipe and check that the run neither stalls nor reports
an xAI error. Expected signals: exit 0, no `api.x.ai` in the network path, no
settings-fetch log line, and the `xAI surfaces disabled` line when
`GROK_XAI_ENABLED=0`.

### Isolated development profile

Development must never touch `~/.grok`. Every command starts from the
canonical environment:

```sh
export GROK_HOME="${HOME}/.grokdev"
export GROK_LEADER_SOCKET="${GROK_HOME}/leader.sock"
export GROK_DISABLE_AUTOUPDATER=1
```

Focused checks, one helper at a time (they share `./target-dev`):

```sh
./grok-check.sh -p xai-grok-shell
./grok-test.sh -p xai-grok-shell -- remote_fetch settings_fetch_budget
./grok-test.sh -p xai-grok-pager -- headless
```

Direct `cargo` only when a helper cannot express the operation, and only after
`source ./grok-dev-env.sh` (that script now works under zsh too, per
`9fa2e3dd`):

```sh
source ./grok-dev-env.sh
cargo build -p xai-grok-pager-bin
```

Run the freshly built binary with `--no-leader` so a stale leader cannot
substitute older code, and `--no-auto-update` so the check is local:

```sh
./target-dev/debug/xai-grok-pager --no-leader --no-auto-update \
  -p "Reply with exactly: OK"
```

Headless calls can reach a configured gateway and consume credentials or
quota from the development profile — expect that, and do not point a
verification run at a production gateway by accident.