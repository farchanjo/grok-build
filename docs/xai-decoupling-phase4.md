# Phase 4 — Removing the Last xAI Dependencies

Status: done — landed on `fcustom` as `503900f0`, `73823e18`, `d6ab4514`,
`bf6a7dce`, `4764110f`. Deviations from this plan are recorded in section 4.
Predecessor: `docs/xai-decoupling.md` (Phases 1-3, committed `ab617625`..`b9ac27a7`)
Language: all authored artifacts in en-US

## 0. Why this phase exists

Phases 1-3 made the *chat* path work with no xAI account and no xAI
reachability, and made the failure mode loud instead of silent. What remains is
everything the run still reaches for when chat is already local: the auxiliary
tools, the relay, the managed-config sync, and a handful of pager commands.
`docs/xai-decoupling.md` section 5 lists them; this phase closes them.

The end state is one sentence: **with `GROK_XAI_ENABLED=0`, a plain bearer and a
base URL, no code path in the binary opens a socket to `x.ai` unless the user
explicitly asked for an xAI-hosted surface.**

Non-goals for this phase:

- Deleting xAI code (that is the separate "surface removal" plan for
  marketplace/plugins/hooks).
- A TUI settings group for every new key. Phase 4 resolves from environment and
  the raw config table; the settings registry follows in Phase 5.
- Synthesizing an endpoint when the user configured none.

## 1. Workstreams

Each workstream is owned by exactly one agent, has a disjoint file set, and
lands as its own commit. No agent runs a workspace-wide check; the integration
pass (Phase 4.E) does that once.

### A — `web_search` without xAI

Today `web_search` is a single wire shape: `POST {base_url}/responses` with an
xAI Responses payload (`web_search/client.rs:141`, `:232`). A third-party
gateway that only speaks `/chat/completions` fails the tool.

Deliverable: a backend trait plus three HTTP backends that need no xAI
credential and no model route.

- `web_search/backends/mod.rs` — `trait WebSearchBackend`, shared
  `SearchRequest`/`SearchResult`, per-backend `is_configured()`.
- `web_search/backends/xai.rs` — today's client, wire-identical.
- `web_search/backends/searxng.rs` — keyless `GET {base}/search?format=json`.
- `web_search/backends/tavily.rs` — `POST {base}/search`, `api_key` header.
- `web_search/backends/brave.rs` — `GET {base}/res/v1/web/search`, `X-Subscription-Token`.
- `web_search/factory.rs` — `resolve_search_backend(env, raw_config_table)`.

Selection precedence: `GROK_SEARCH_PROVIDER` > `[search] provider` > `xai`.
`GROK_SEARCH_BASE_URL` > `[search] base_url`; `GROK_SEARCH_API_KEY` >
`[search] api_key_env`'s value. Read the raw config table the way
`xai-grok-voice/src/config.rs:67-83` does, so no typed-config churn is required.

Hard requirements:

- Provider `xai` must be byte-identical to today (existing tests are the proof).
- With a non-xAI provider, the tool registers even when no xAI credential and
  no `models.web_search` route resolve.
- A backend with a missing base URL or key reports why in the tool result, not
  a silent `Disabled`.

Files: `xai-grok-tools/src/implementations/web_search/**`,
`xai-grok-shell/src/session/acp_session_impl/spawn.rs`,
`xai-grok-agent/src/builder.rs`,
`xai-grok-workspace/src/session/tool_config.rs`.

### B — Nothing else dials xAI

Three ordering/latching defects let a run touch xAI after the switch is off.

1. **The relay wins over the switch.** `apply_xai_switch` runs inside
   `agent::init::bootstrap` (`init.rs:41`), but the relay is configured earlier
   on the leader and headless paths, so a present grok.com session still opens
   the `grok.com` WebSocket with `GROK_XAI_ENABLED=0`. Fix the ordering (apply
   the switch at the earliest entry point that can see the config), or make
   `RelayConfig::for_session` (`relay.rs:71`) consult `xai_enabled()` directly
   if the ordering cannot be closed for every path. Verify on both the leader
   and headless paths; do not add a redundant check if the hoist already covers
   them.
2. **The managed-config deployment sync is not gated by the switch.**
   `managed_config.rs:469` and the prefetch-thread sync ride `[features]
   managed_config` (default true), so an off-xAI run still fetches deployment
   config. `GROK_XAI_ENABLED=0` must disarm it; `managed_policy_gate` still
   fails closed.
3. **Loopback is first-party.** `is_cli_chat_proxy_url` treats
   `http://localhost:*` as a proxy host, so a local vLLM gets `x-grok-*`
   headers. Add an explicit opt-out (env + `[endpoints]` key) and keep today's
   default.

Files: `xai-grok-shell/src/agent/{relay,app,init,models}.rs`,
`xai-grok-shell/src/managed_config.rs`,
`xai-grok-shell-base/src/util/mod.rs`.

### C — xAI-only pager surfaces degrade with a sentence, not a stack trace

`/share`, `/billing`, `/usage`, and the `x.ai/cloud/*` handlers all require
`is_xai_auth`. With the switch off they must say so in one line and keep the
session usable.

Also closes the known sharp edge from `docs/xai-decoupling.md`: the TUI catches
`authenticate` errors and shows the login-aware welcome flow, so the new
no-credential remedy is invisible outside `grok -p`
(`xai-grok-pager/src/acp/mod.rs:740-752`). Surface it.

Files: `xai-grok-pager/src/extensions/{share,billing,auth_gate}.rs`,
`xai-grok-shell/src/agent/mvp_agent/acp_agent.rs` (cloud handlers only),
`xai-grok-pager/src/acp/mod.rs`.

### D — Media and voice stop assuming `api.x.ai` serves them

`image_gen`, `image_edit`, `video_gen`, and voice STT all resolve
`endpoints.xai_api_base_url` (`agent_ops.rs:1245`, `:1298`;
`xai-grok-voice/src/config.rs:56`) and then assume the endpoint implements
`/images/*`, `/videos/*`, and the `/stt` WebSocket. Against a local gateway the
first two may work (they are OpenAI-shaped) and the last two usually do not.

Deliverable:

- Per-surface base URL and model resolution, so a user can point Imagine at a
  gateway that serves it while chat goes elsewhere (env + `[tools.*]`/`[voice]`
  keys, defaulting to today's behavior).
- `/images/*` request and response tolerance for the OpenAI shape, so an
  OpenAI-compatible endpoint works without a translation layer.
- When the endpoint cannot serve a surface (404/405 on the probe, or an
  explicit `provider = "unsupported"`), the tool result names the remedy
  instead of returning a raw status.

Files: `xai-grok-tools/src/implementations/grok_build/{image_gen,video_gen}/**`,
`xai-grok-voice/src/config.rs`,
`xai-grok-shell/src/agent/mvp_agent/agent_ops.rs`.

### E — Integration (single owner, after A-D report)

Narrow checks per touched crate, then the off-xAI battery
(`grok-off-xai-check.sh`, committed in `feat(docs)`; at the time it lived in
`/tmp` as `final_check.sh`), then one tmux drive-through. Full-workspace check
once.
Commits are per workstream, made by each owning agent with explicit pathspecs.

## 2. Shared rules for every agent

- Isolated profile: `bash -c 'set +e; unset GROK_HOME GROK_LEADER_SOCKET; source
  ./grok-dev-env.sh; set +e; <cmd>'`. `GROK_HOME=~/.grokdev`.
- One Cargo helper at a time; four agents share `./target-dev`, so lock waits
  are expected. Never delete `.cargo` locks, never set a private
  `CARGO_TARGET_DIR`.
- `./grok-check.sh -p <crate>` for the narrow loop; `./grok-test.sh -p <crate>
  -- <filter>` for the family that covers the change.
- Never `git add -A` (other agents' edits are in the tree). Commit with
  explicit pathspecs; never touch `Cargo.lock`.
- Do not edit `docs/xai-decoupling.md` — the docs agent rewrites it in Phase 4.E.
- If a file outside your list must change, keep the edit surgical and say so in
  the report.

## 3. Acceptance for the phase

Each row carries the command that proves it and what that command actually
printed. Rows marked **measured** were re-run against the committed tree on
2026-09-15; rows marked **quoted** are taken from the commit message named in
the row and were not re-measured.

| Check | Target | How it was proved | Observed |
| --- | --- | --- | --- |
| `GROK_SEARCH_PROVIDER=searxng` with no xAI credential | `web_search` returns results | **Measured.** `./grok-test.sh -p xai-grok-tools -- web_search` | 69 tests run, 69 passed, 3002 skipped, exit 0. The backends run against `wiremock`, so this proves the wire and the credential-free registration, not a live SearXNG. |
| `GROK_XAI_ENABLED=0` with a grok.com session present | no `grok.com` WebSocket opened | **Measured.** `bash grok-off-xai-check.sh`, scenarios 3 and 4 | Scenario 3 (session present, switch off) records no `relay_connecting`; scenario 4 is the control (same session, switch on) and does record it, so scenario 3 cannot pass vacuously. |
| `GROK_XAI_ENABLED=0`, no managed config on disk | no deployment-config fetch | **Quoted** from `d6ab4514`. Needs `GROK_DEPLOYMENT_KEY` plus a blackholed `GROK_MANAGED_CONFIG_URL`; the battery does not cover it. | No socket with the switch off, two sockets with it on. |
| `/share`, `/billing` with no xAI auth | one-line message, session survives | **Measured.** `./grok-test.sh -p xai-grok-shell -- auth_gate billing cloud_gate no_credential` | 23 tests run, 23 passed, 7550 skipped, exit 0. Includes `non_xai_credential_refusal_is_the_same_sentence`, `billing_refuses_without_any_credential` and the three `cloud_*_refuses_*` cases. |
| Off-xAI battery with xAI blackholed | unchanged from Phase 3 (exit 0, ~5 s) | **Measured.** `bash grok-off-xai-check.sh` (needs a built `xai-grok-pager` and a grok.com session in `~/.grokdev/auth.json` for the two session scenarios) | 15 assertions passed, 0 failed, exit 0 — under macOS bash 3.2.57 and under Homebrew bash 5.3.9. Wall clock is minutes, not the ~5 s the target guessed: the blackhole makes each xAI dial burn its full connect timeout. |
| xAI-present path | byte-identical wire for chat, search, and Imagine | **Quoted.** No live xAI endpoint is reachable from this host. | Wire shapes asserted in tests; not end-to-end proven against xAI. |

Supporting families re-run for this revision, all exit 0:
`./grok-test.sh -p xai-grok-shell -- xai_switch first_party_loopback` →
10 tests run, 10 passed, 7563 skipped (includes
`relay_gate_loses_to_the_switch_applied_at_the_entry_points` and
`is_fetch_enabled_is_disarmed_by_the_xai_switch`);
`./grok-test.sh -p xai-grok-tools -- media_endpoint` →
11 tests run, 11 passed, 3060 skipped;
`./grok-test.sh -p xai-grok-shell -- auth_gate billing cloud_gate no_credential` →
23 tests run, 23 passed, 7550 skipped.

Three residuals this table does **not** cover, recorded rather than implied:

- **No relay end-to-end on the leader path.** The headless path has
  `tests/xai_switch_relay_e2e.rs`; the leader path is covered by the entry-point
  replay test and by manual runs only.
- **No live search backend.** SearXNG, Tavily and Brave are exercised against
  `wiremock`; the battery talks to a stub gateway and to nothing else.
- **No settings-registry entry.** `[search]`, `[tools.image_gen|image_edit|video_gen]`
  and `[voice].api_base` are absorbed as raw `toml` on the typed config (declared
  so `serde_ignored` does not warn) and resolved from env plus that raw table, but
  `settings/defs.rs` / `registry.rs` still list none of them: the settings sheet
  does not show them and `/config` cannot edit them. Deferred to Phase 5 by
  design, and therefore not testable here.

## 4. Deviations from the plan that actually happened

The phase landed as five commits. This section records where the plan and the
code diverged; nothing here changes the end state described in section 0.

1. **Workstream C's file paths were wrong.** The plan listed
   `xai-grok-pager/src/extensions/{share,billing,auth_gate}.rs`. All three live
   in `crates/codegen/xai-grok-shell/src/extensions/`; the pager contributes
   only `src/acp/mod.rs` and `src/app/event_loop.rs` to that workstream.
   `docs/xai-decoupling.md` section 3 carried the same wrong paths and is
   corrected.

2. **The relay ordering was already fixed; only the test was missing.**
   Workstream B item 1 asked for the hoist, but `8ede5af7` had already put
   `apply_xai_switch` at the top of `run_leader`, `run_headless_inner` and
   `run_stdio_agent`, and `d6ab4514` left `agent/app.rs` byte-identical. What
   landed is the coverage: the old `relay_gate_loses_...` case set the process
   flag itself, so a dropped hoist passed it. The new unit test replays the
   entry-point order with the switch read from the environment, and
   `tests/xai_switch_relay_e2e.rs` drives a real `agent headless`. No redundant
   `xai_enabled()` check was added to `RelayConfig::for_session`, because the
   hoist covers every relay construction site.

3. **New keys resolve from the environment plus the raw config table, not typed
   settings.** `[search]`, `[tools.image_gen]` / `[tools.image_edit]` /
   `[tools.video_gen]` and `[voice].api_base` are read as raw `toml` tables, the
   way `VoiceConfig::from_config_table` already read `[voice]`. Consequence: no
   typed-config field, no settings-registry entry and no settings UI for any of
   the new keys. The registry follows in Phase 5, as the non-goals said.
   (Later, `69bc9f28` added raw `search` / `tools` absorption on the config
   struct so the tables stop tripping `serde_ignored`. They are still raw
   `toml::Value` and still absent from the settings registry.)

4. **Two changes outside the declared file set were needed.**
   `xai-grok-config/src/lib.rs` gained a `pub use toml;` re-export so the tools
   crate can name the raw table without a new dependency edge;
   `xai-grok-shell/Cargo.toml` and `xai-grok-shell-base/Cargo.toml` gained the
   `[[test]]` registration (the crate sets `autotests = false`) and the
   `serial_test` dev-dependency the process-global flag tests need.
   `Cargo.lock` moved by one line.

5. **`4764110f` is narrower than "six failures".** Five test families were
   repaired across three files; the two `resolve_model_list_*` cases ride in
   `d6ab4514` because they share a file with the loopback key. All six were
   stale assertions, none a regression.

6. **There is no separate integration commit.** Workstream E's narrow per-crate
   checks and the blackhole battery ran inside the owning workstreams, so the
   phase has no workspace-wide-check commit of its own.