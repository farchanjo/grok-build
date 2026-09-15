# Running This Fork With Zero xAI Dependency

A practical reference for running this fork against a third-party or local
OpenAI-compatible gateway, with every xAI-hosted surface switched off.

Scope and status:

- Phases 1-3 landed on `fcustom` in `ab617625` (`feat(xai-free): run without an
  xAI account, by default`) and `a8c4d2ea` (`feat(xai-free): one switch for xAI
  surfaces, identity from the endpoint`), plus the follow-ups `dc22e438`,
  `44d7a2a4`, `bcd261e3`, `a68c2b6a`, `8ede5af7` and `4e3e417a`.
- Phase 4 landed on `fcustom` in five commits: `503900f0` (per-surface media
  endpoints, OpenAI image wire, actionable failures), `73823e18` (pluggable
  search backends with no xAI credential), `d6ab4514` (the switch outranks the
  relay, the managed-config sync and the loopback identity),
  `bf6a7dce` (xAI-only pager surfaces refuse with one sentence; the TUI shows
  the no-credential remedy) and `4764110f` (six stale test failures repaired).
  Section 3 is restructured around them: the surfaces that used to "still touch
  xAI" now go quiet under the switch, and what remains hard is listed
  separately.
- All `file:line` references were re-derived against the committed tree after
  Phase 4 landed (`4764110f`, plus the rustfmt-only `a1d0e670` on top). The
  Phase 4 measurements in section 4 were taken at `4764110f`. Line numbers
  drift; the paths do not.
- Everything below was cross-checked against the code. Section 4 marks which
  results were re-measured for this revision and which are quoted from a commit
  message.

## 0. Vocabulary

Phase 4 overloaded a handful of names. They are not synonyms, and reading one
as the other is the easiest way to misconfigure a run.

| Name | Meaning in one place | Meaning in another | The distinction that matters |
| --- | --- | --- | --- |
| `provider` | For search, a **backend id**: `xai`, `searxng`/`searx`, `tavily`, `brave`/`brave-search` | For media, a **wire policy**: `auto`, `xai`, `openai`, `unsupported` | `[search] provider` picks who answers; `[tools.image_gen] provider` picks how the request is shaped and whether the surface runs at all. Neither value set is valid for the other key. |
| `GROK_SEARCH_MODEL` | Overrides the `[models] web_search` route model, for the `xai` backend **only** | — | A non-xAI backend carries its own model and ignores both this key and the xAI credential. It re-points a model; it does not re-point a route. |
| `GROK_IMAGE_*` | The imagine surface's endpoint, model and wire: `GROK_IMAGE_BASE_URL`, `GROK_IMAGE_MODEL`, `GROK_IMAGE_PROVIDER` | Two unrelated families under the same prefix: the **enable** flags `GROK_IMAGE_GEN` / `GROK_IMAGE_EDIT` (with `[features] image_gen` as the config twin), and the legacy vision route `GROK_IMAGE_DESCRIPTION_MODEL` | Setting a base URL does not enable anything, and setting the enable flag does not move an endpoint. `GROK_IMAGE_EDIT` defaults to enabled, so absence is not off. |
| `xai` as a value | In `provider`, the xAI wire shape or backend — **not** "reachable" | `GROK_XAI_ENABLED` gates xAI *identity* | `provider = "xai"` dials whatever the base URL says, so it can point at a non-xAI host and still be labelled `xai`. The switch is the only thing that decides whether xAI is on. |
| `unsupported` / `off` / `false` | `provider = "unsupported"` (aliases `none`, `off`) silences one media surface before any HTTP call | `GROK_IMAGE_GEN=0` disables the tool; `GROK_XAI_ENABLED=0` gates identity process-wide | Three different blast radii: one surface, one tool, one process. |
| `base_url` | `[model.<id>] base_url` is a chat endpoint | `[tools.<surface>] base_url` is a media endpoint; `[search] base_url` is a search backend; `[endpoints].xai_api_base_url` is the fallback the xAI-hosted surfaces resolve to | Four scopes, four keys. Moving chat does not move the tools, which is the whole point of the per-surface keys. |

## 1. The switches

Every key in this section resolves **env > config > default**. Blank and
whitespace-only values fall through to the next tier rather than pinning an
empty string, and every resolver strips trailing slashes so callers can append a
path.

### The xAI kill switch

| Setting | Type | Default | Precedence | Implemented at |
| --- | --- | --- | --- | --- |
| `GROK_XAI_ENABLED` | strict bool | `true` | env > `[xai] enabled` config key | const `xai-grok-shell-base/src/util/mod.rs:101`; resolved `:117`/`resolve_xai_enabled_from` `:127`; applied `xai-grok-shell/src/agent/init.rs:42` (top of `bootstrap`, before `resolve_config`) and `:163` (`apply_xai_switch`); consumed `xai-grok-shell/src/auth/model.rs:148-151` |
| `[xai] enabled` | bool | `true` | Config twin; the strict env tier wins over it | `xai-grok-shell/src/agent/config.rs:1279` (`XaiConfig`); applied `xai-grok-shell/src/agent/init.rs:164` |

"Strict bool" means the parser (`xai_grok_config::env_bool`) accepts
`1/true/yes/on/enabled` and `0/false/no/off/disabled`; anything else —
including a blank value or a typo — falls through to the next tier instead of
silently flipping the switch. `GROK_XAI_ENABLED` is deliberate about this.

`GROK_XAI_ENABLED` has exactly one production consumer:
`GrokAuth::is_xai_auth` returns `false` while it is off
(`xai-grok-shell/src/auth/model.rs:148-151`). That single choke point is what
quiets the relay, share links, billing, cloud sandboxes, Writeback, remote
sessions/workspaces, managed MCP, managed-config fetching, trace upload and
telemetry identity even when an xAI credential is present.

Because the relay is picked *before* `bootstrap`, the switch is applied at the
top of the three agent entry points as well — `agent/app.rs:339`
(`run_stdio_agent`), `:497` (`run_headless_inner`), `:1013` (`run_leader`) —
each immediately before the relay gate. `apply_xai_switch` is idempotent and is
called again from `init_process` (`agent/init.rs:189`).

### Search backend (`web_search`)

`web_search` no longer implies an xAI route. With **nothing** set below, the
factory returns `None` and today's `models.web_search` route resolution applies
byte-for-byte. A non-xAI provider ignores the xAI route entirely, so the tool
registers with no xAI credential and no `models.web_search` route.

| Env var | Config twin | Default | Precedence | Implemented at |
| --- | --- | --- | --- | --- |
| `GROK_SEARCH_PROVIDER` | `[search] provider` | `xai` | env > config > `xai` | `xai-grok-tools/src/implementations/web_search/factory.rs:28`; resolve `:157-166` |
| `GROK_SEARCH_BASE_URL` | `[search] base_url` | unset | env > config | `:30`; resolve `:169-170` |
| `GROK_SEARCH_API_KEY` | `[search] api_key_env` — names the *env var* holding the key, so no secret lands in `config.toml` | unset | env > the value of the named env var | `:32`, `:43`; resolve `:171-181` |
| `GROK_SEARCH_MODEL` | `[search] model` | unset | env > config; `xai` backend only | `:34`; resolve `:182-183` |

Accepted provider ids are case-insensitive: `xai` (default), `searxng`/`searx`,
`tavily`, `brave`/`brave-search`. Wire shapes
(`web_search/backends/mod.rs:1-11`):

| Provider | Request | Auth |
| --- | --- | --- |
| `xai` | `POST {base}/responses` with a server-side `web_search` tool | bearer, unchanged |
| `searxng` | `GET {base}/search?format=json` | none — keyless |
| `tavily` | `POST {base}/search` | `Authorization: Bearer` |
| `brave` | `GET {base}/res/v1/web/search` | `X-Subscription-Token` |

Deliberate behaviors worth knowing: a lone knob (say, only a base URL) already
leaves the default xAI route, so the selection is never half-applied; an
*unknown* provider id still builds a config, so the tool registers and the
error names the accepted ids instead of the tool silently disappearing; and a
backend missing its base URL or key constructs anyway and reports which knob to
set in the tool result (`web_search/backends/mod.rs:186-196`,
`UnconfiguredBackend`). SearXNG instances must have `format: json` enabled; an
HTML answer says so.

### Media surfaces (`image_gen`, `image_edit`, `video_gen`)

Each surface has its own endpoint, model and wire shape, so imagine can point at
a gateway while chat goes elsewhere. With every key unset the resolved values
are exactly what the tools used before Phase 4.

| Env var | Config twin | Default | Precedence | Implemented at |
| --- | --- | --- | --- | --- |
| `GROK_IMAGE_BASE_URL` | `[tools.image_gen] base_url` | `[endpoints].xai_api_base_url` | env > config > endpoints | `xai-grok-tools/src/implementations/grok_build/media_endpoint.rs:56`; resolve `xai-grok-shell/src/agent/mvp_agent/agent_ops.rs:1327-1333` |
| `GROK_IMAGE_MODEL` | `[tools.image_gen] model` | remote `image_gen_model_override`, else the client default | env > config > remote override | `media_endpoint.rs:64`; `agent_ops.rs:1343-1348` |
| `GROK_IMAGE_PROVIDER` | `[tools.image_gen] provider` | `auto` | env > config > `auto` | `media_endpoint.rs:72`; `agent_ops.rs:1366-1374` |
| `GROK_IMAGE_EDIT_BASE_URL` | `[tools.image_edit] base_url` | the resolved image base URL | env > config > image base | `media_endpoint.rs:57`; `agent_ops.rs:1335-1341` |
| `GROK_IMAGE_EDIT_MODEL` | `[tools.image_edit] model` | the resolved image model, else the client default | env > config > image model | `media_endpoint.rs:65`; `agent_ops.rs:1359-1365` |
| `GROK_VIDEO_BASE_URL` | `[tools.video_gen] base_url` | `[endpoints].xai_api_base_url` | env > config > endpoints | `media_endpoint.rs:58`; `agent_ops.rs:1438-1444` |
| `GROK_VIDEO_MODEL` | `[tools.video_gen] model` | the client default | env > config | `media_endpoint.rs:66`; `agent_ops.rs:1452-1458` |
| `GROK_VIDEO_PROVIDER` | `[tools.video_gen] provider` | `auto` | env > config > `auto` | `media_endpoint.rs:73`; `agent_ops.rs:1446-1450` |

`GROK_IMAGE_PROVIDER` covers both image surfaces. Provider values
(`media_endpoint.rs:119-129`): `auto` (default), `xai` (alias of `auto`),
`openai`/`open_ai`/`open-ai`, and `unsupported`/`none`/`off`. An unrecognized
value warns and falls back to `auto`, so a typo cannot disable a surface.

What the provider value changes: `openai` sends the OpenAI request fields
(`size` instead of `aspect_ratio`/`resolution`; multipart for edits). The
*response* decoder accepts both the xAI and the OpenAI envelope for every value,
including `auto`. `unsupported` short-circuits before any HTTP call with a
remedy naming the key to set. A 404/405/501 from a media endpoint produces the
same style of message (`media_endpoint.rs:256-289`) instead of a bare status.

### Voice dictation (STT)

STT is its own surface: a gateway that serves chat, or even the
OpenAI-compatible `/images/*` routes, does not necessarily serve the xAI `/stt`
WebSocket.

| Env var | Config twin | Default | Precedence | Implemented at |
| --- | --- | --- | --- | --- |
| `GROK_VOICE_BASE_URL` | `[voice].api_base` | `[endpoints].xai_api_base_url`, else `https://api.x.ai` | env > `[voice].api_base` > `[endpoints].xai_api_base_url` > resolved endpoints base > built-in default | `xai-grok-voice/src/config.rs:15`; resolve `:74-116` |

Voice keeps its TLS-only rule: a plaintext `http://` / `ws://` base is rejected
with a message naming `GROK_VOICE_BASE_URL` (`config.rs:130-148`).

### Endpoint identity (loopback)

`http://localhost:<port>/v1` is first-party by default, because local mock
servers should keep behaving like the cli-chat-proxy. A real local gateway
(vLLM, LiteLLM, Ollama) is a *custom* endpoint and should not receive
`x-grok-*` / `X-XAI-Token-Auth` headers.

| Env var | Config twin | Default | Precedence | Implemented at |
| --- | --- | --- | --- | --- |
| `GROK_FIRST_PARTY_LOOPBACK` | `[endpoints] first_party_loopback` | `true` (loopback is first-party) | env > config > `true` | const `xai-grok-shell-base/src/util/mod.rs:144`; resolve `:170-174`; applied `xai-grok-shell/src/agent/init.rs:165`; consumed `xai-grok-shell-base/src/util/mod.rs:65` |

Setting it to `0` moves `localhost`, `127.0.0.0/8` and `::1` out of the
first-party trust set, so the identity derives `Custom`. The explicit
`cli_chat_proxy_base_url` arm is unaffected: a run that *names* the proxy still
gets a first-party identity.

### Fetch, update and side channels

| Setting | Type | Default | Precedence | Implemented at |
| --- | --- | --- | --- | --- |
| `GROK_REMOTE_FETCH` / `[features] remote_fetch` | strict bool | falls through to config layers; final default `true` | env > requirements (MDM > system > user) > managed > system managed > user `config.toml` | const `xai-grok-shell/src/util/config/resolve/features.rs:5`; resolve `:44`; env tier `:69`; config twin `xai-grok-shell/src/agent/config.rs:5589`; requirement pin `xai-grok-shell/src/config/mod.rs:1307` |
| `GROK_SETTINGS_FETCH_TIMEOUT_SECS` | integer seconds | `10` (`0`/blank/junk keeps the default) | env | const `xai-grok-shell/src/remote/client.rs:18`; used by `fetch_settings_blocking` `:589` |
| `GROK_CHANGELOG_OFFLINE` | presence flag | unset → fetch CDN | any non-empty value except `0` skips the CDN | `xai-grok-shell-base/src/util/changelog.rs:204`, used `:103` |
| `GROK_DISABLE_AUTOUPDATER` | truthy flag | unset → check for updates | env | `xai-grok-pager-bin/src/main.rs:2660` |
| `GROK_TELEMETRY_ENABLED` / `[features] telemetry` | bool or mode | falls through to config | env > config | `xai-grok-shell/src/agent/config.rs:2627`, `:2656`; config twin `:5592` |
| `GROK_FEEDBACK_ENABLED` / `[features] feedback` | bool | falls through to config | env > requirements > config > remote settings | `xai-grok-shell/src/agent/config.rs:2749`; config twin `:5607` |
| `GROK_TELEMETRY_TRACE_UPLOAD` / `[telemetry] trace_upload` | bool | inherits the telemetry toggle when unset | env > `[telemetry] trace_upload` | `xai-grok-telemetry/src/config.rs:172`; resolve `xai-grok-shell/src/agent/config.rs:2683` |

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
| `GROK_MODELS_BASE_URL` | Turns the endpoint into a "custom endpoint": inference goes there, the model list is read from `{base}/models`, and the bundled xAI catalog is skipped. It also stops the api-key route from pointing at `api.x.ai`. | `config.rs:575`, `:326-331`, `:557-562`, `:3869-3877` |
| `GROK_XAI_ENABLED=0` | Turns every xAI-hosted surface off process-wide, credential or not: the relay, the managed-config fetch, share/billing/usage/cloud, trace upload and telemetry identity. Section 3 lists each one. | `util/mod.rs:101`, `:127`, `auth/model.rs:148-151`, `managed_config.rs:481-501` |
| `GROK_REMOTE_FETCH=0` | Disarms the startup model-catalog fetch and the `/v1/settings` fetch (a custom `GROK_MODELS_BASE_URL` catalog still loads). Startup does not wait on `cli-chat-proxy.grok.com` for either. | `features.rs:44`, callers `agent/mvp_agent/agent_ops.rs:1050`, `agent/models.rs:1956`, `agent/subscription_check.rs:140` |
| `GROK_CHANGELOG_OFFLINE=1` | Skips the `https://x.ai/cli/changelogs` CDN and reads the local cache only. | `changelog.rs:15`, `:103`, `:204` |
| `GROK_DISABLE_AUTOUPDATER=1` | No update check. `--no-auto-update` does the same thing per-run. | `main.rs:2660` |
| telemetry / feedback off | No product-analytics events and no feedback endpoint calls. | `config.rs:2627`, `:2749` |

The first four lines are the whole off-xAI configuration; the remaining lines
only remove side channels that would otherwise still be attempted.

Two optional additions, if the run uses the auxiliary tools:

```sh
# 4. Search without an xAI route (SearXNG is keyless).
export GROK_SEARCH_PROVIDER=searxng
export GROK_SEARCH_BASE_URL='http://localhost:8888'

# 5. Media, only if your gateway actually serves those paths.
export GROK_IMAGE_BASE_URL='https://gateway.example/v1'
export GROK_VIDEO_PROVIDER=unsupported
export GROK_VOICE_BASE_URL='https://gateway.example'   # must be https

# 6. A local gateway is custom, not first-party.
export GROK_FIRST_PARTY_LOOPBACK=0
```

Without step 4, `web_search` falls back to the xAI Responses wire and needs an
xAI route; without step 5, the media tools inherit `[endpoints].xai_api_base_url`
and fail on a gateway that does not serve `/images/*` or `/videos/*` — with a
message that names the key.

## 3. What still touches xAI

### Closed by the switch

These used to be part of the "still touches xAI" story. With
`GROK_XAI_ENABLED=0` each one goes quiet *before* a request is attempted; the
column states exactly what the switch does.

| Surface | What the switch does | Where |
| --- | --- | --- |
| grok.com relay WebSocket | The switch is applied at the top of `run_leader`, `run_headless_inner` and `run_stdio_agent`, before the relay gate. `RelayConfig::for_session` gates on `is_xai_auth`, which the switch has already forced false, so a present grok.com session no longer opens the WebSocket. The leader keeps serving clients over IPC; headless serves the agent over stdio after `No xAI session: serving the agent over stdio (ACP); the grok.com relay is off.` on stderr. | hoist `agent/app.rs:339`, `:497`, `:1013`; gate `agent/relay.rs:71`; fallback `agent/app.rs:624-631`; pinned by `agent/relay.rs:1208` and `tests/xai_switch_relay_e2e.rs` |
| Managed-config deployment sync | `is_fetch_enabled` ANDs the switch with `[features] managed_config`, so an off-xAI run does not dial `cli-chat-proxy` for deployment config even when `managed_config = true`. It resolves from env/config rather than the process flag, because the startup prefetch thread decides before the entry points apply the switch. The *enforcement* gate is untouched: `managed_policy_gate` still fails closed on an on-disk policy. | `managed_config.rs:481-501`; prefetch caller `agent/models.rs:2131`; gate `agent/init.rs:44` |
| Loopback identity | `apply_xai_switch` lands the loopback opt-out next to the kill switch, so every hoisted entry point sets it before any identity derivation. Loopback stops deriving a first-party identity and stops sending `x-grok-*` / `X-XAI-Token-Auth`. Off by default, so a local mock server is unchanged unless you opt out. | `agent/init.rs:165-181`; `xai-grok-shell-base/src/util/mod.rs:61-68`, `:150-174` |
| `/share` | Refuses with `auth_required` and the one-liner `` `/share` needs a grok.com session: connect xAI in /providers to authenticate. `` The credential is untouched and a second call answers identically. No grok.com URL is built. | gate `xai-grok-shell/src/extensions/auth_gate.rs:70-79`; caller `extensions/share.rs:176-180` |
| `/billing` | Same refusal, wording `` `/billing` needs … ``. No credits request is sent. | `extensions/billing.rs:203-206` |
| `/usage` | Same refusal, wording `` `/usage` needs … ``. No auto-top-up request is sent. | `extensions/billing.rs:293-296` |
| `x.ai/cloud/*` | All five handlers (`terminate`, `env/list`, `env/create`, `env/update`, `env/delete`) gate *before* building a sandbox client, wording `Cloud sandboxes need …`, so the caller gets a named reason instead of a connection error. | `agent/mvp_agent/acp_agent.rs:3894`, `:3929`, `:3957`, `:4017`, `:4080` |
| TUI no-credential remedy | `initialize` advertises `noCredentialRemedy` (the `no_credential_message` sentence verbatim); the pager carries it on `AcpConnection::auth_remedy` and renders it — the welcome error line when the login flow is on (login menu untouched), a scrollback/status line otherwise. Healthy runs stay silent. | `agent/mvp_agent/acp_agent.rs:714` (from `:171`); `xai-grok-pager/src/acp/mod.rs:88`, `:305-307`; `xai-grok-pager/src/app/event_loop.rs:3947-3960` |

### Hard (the run breaks or changes shape without the other side)

| Case | What happens | Where |
| --- | --- | --- |
| No credential, no custom endpoint, model still bound to xAI | Fails before the first request, with the remedy in the message (quoted below) and exit 1 on `grok -p`. Nothing synthesizes a local endpoint. | preflight `agent/mvp_agent/acp_agent.rs:790-796`; message `agent/config.rs:5957-5963`; exit `xai-grok-pager-bin/src/main.rs:2190-2194`, `xai-grok-pager/src/headless.rs:1049-1053`; warning path `agent/config.rs:5863-5874` |
| `grok agent headless` without a grok.com session | Not a failure, but a shape change: the agent is served over stdio (ACP) instead of over the relay, and it exits on stdin EOF when the stdin bridge shuts the ACP input down. A harness that closes stdin ends the process. | fallback `agent/app.rs:624-631`; stdin bridge `agent/app.rs:389-404` |
| A gateway that does not serve `/images/*`, `/videos/*` or the `/stt` WebSocket | The tools still fail — but with a remedy naming `GROK_IMAGE_*` / `GROK_VIDEO_*` / `GROK_VOICE_BASE_URL` and the option to set `provider = "unsupported"`, not with a bare status. | `grok_build/media_endpoint.rs:256-289`; `grok_build/image_gen/mod.rs:276-285`; `xai-grok-voice/src/config.rs:130-148` |
| Managed policy on disk | `managed_policy_gate` fails closed at startup regardless of the switch: a tampered policy must not run unmanaged. Only the *fetch* goes quiet. | `agent/init.rs:44` |
| `x.ai/cloud/*`, `/share`, `/billing`, `/usage` | No local equivalent exists, so they refuse. A run that needs them still needs a grok.com session. | see the table above |

The no-credential message is verbatim
(`agent/config.rs:5957-5963`), with the effective model id interpolated:

```text
No credential for model `<model>`: set GROK_API_KEY (a plain bearer) together with GROK_MODELS_BASE_URL=<endpoint>/v1 for a non-xAI gateway, or sign in with `grok provider connect xai`
```

`grok -p` (single-turn, pager-side) is *not* affected by the relay: it never
used it. That is the path the measurements below exercise.

### Soft (warns or degrades)

| Surface | Behavior when xAI is off | Where |
| --- | --- | --- |
| Startup settings fetch | Advisory only. 10 s per attempt, 2 attempts, then a process-wide latch so later callers (prefetch thread, shell fallback, post-auth refresh) return immediately. Disarmed entirely by `GROK_REMOTE_FETCH=0`. | `remote/client.rs:16`, `:18`; callers `agent/mvp_agent/agent_ops.rs:1050`, `agent/models.rs:1956`, `agent/subscription_check.rs:145` |
| Bundled model catalog | Used only when no custom endpoint is configured; with `GROK_MODELS_BASE_URL` set it is skipped. | `agent/config.rs:3869-3877`, `:4154` |
| Managed MCP | The managed server list still comes from the backend. | `xai-grok-config-types/src/lib.rs:679` |
| Trace upload | Off via `[telemetry] trace_upload` / `GROK_TELEMETRY_TRACE_UPLOAD`; uploads ride first-party auth. | `xai-grok-telemetry/src/config.rs:172`; the gate is asserted in `xai-grok-pager/tests/pty_e2e/storage_upload_parks_on_401_and_drains_after_recovery.rs:19` |

## 4. What is verified

Phase 4 results were re-measured for this revision unless the row says
otherwise. Every command below runs under the canonical isolation recipe:

```sh
bash -c 'set +e; unset GROK_HOME GROK_LEADER_SOCKET; source ./grok-dev-env.sh; set +e; <command>'
```

`GROK_HOME` ends up `${HOME}/.grokdev`, `cargo-nextest` is the runner, and one
helper runs at a time against `./target-dev`.

### Phase 4 (measured at HEAD `4764110f`)

| Check | Command | Observed |
| --- | --- | --- |
| Media endpoint precedence, wire-shape choice, remedy text | `./grok-test.sh -p xai-grok-tools -- media_endpoint` | `10 tests run: 10 passed, 3059 skipped`, exit 0 |
| Search backends, factory precedence, request shaping | `./grok-test.sh -p xai-grok-tools -- web_search` | `68 tests run: 68 passed, 3001 skipped`, exit 0 |
| Tool registers with no xAI route; missing key reported | `./grok-test.sh -p xai-grok-agent -- external_search_backend` | `2 tests run: 2 passed, 592 skipped`, exit 0 |
| Session context carries the selected backend; unset keeps the old path | `./grok-test.sh -p xai-grok-workspace -- external_search_provider default_search_selection` | `2 tests run: 2 passed, 1563 skipped`, exit 0 |
| Loopback opt-out, identity and header derivation | `./grok-test.sh -p xai-grok-shell-base -- first_party_loopback resolve_first_party is_cli_chat_proxy is_xai_api` | `9 tests run: 9 passed, 66 skipped`, exit 0 |
| Relay loses to the switch; managed sync disarmed; loopback and identity | `./grok-test.sh -p xai-grok-shell -- xai_switch first_party_loopback is_fetch_enabled provider_identity url_derived_headers_follow` | `13 tests run: 13 passed, 7548 skipped`, exit 0 |
| `/share`, `/billing`, `/usage`, cloud refuse in one line, session survives | `./grok-test.sh -p xai-grok-shell -- auth_gate billing cloud_gate` | `20 tests run: 20 passed, 7541 skipped`, exit 0 |
| No-credential remedy text | `./grok-test.sh -p xai-grok-shell -- no_credential` | `3 tests run: 3 passed, 7558 skipped`, exit 0 |
| STT endpoint precedence, TLS rule, handshake messages | `./grok-test.sh -p xai-grok-voice` | `53 tests run: 53 passed, 1 skipped`, exit 0 |
| Pager carries and renders the remedy | `./grok-test.sh -p xai-grok-pager -- auth_remedy hydrate_connection` | `3 tests run: 3 passed, 8608 skipped`, exit 0 |

All ten commands exited 0 with zero failures. A `skipped` count is the rest of
the crate's suite, filtered out by the test-name filter.

### Phase 4 (quoted from commit messages, not re-measured)

- `d6ab4514` — with the hoist removed, `tests/xai_switch_relay_e2e.rs` fails with
  `the headless entry point must announce the relay is off` while stderr shows
  `relay_connecting ws_url=ws://127.0.0.1:1/ws attempt=3`; with the hoist, both
  e2e tests pass. Manual leader + headless runs with a grok.com session in
  `~/.grokdev/auth.json`, xAI blackholed: no socket with `GROK_XAI_ENABLED=0`,
  socket present with `=1`; `[xai] enabled = false` in `config.toml` behaves the
  same and `GROK_XAI_ENABLED=1` beats it. Managed sync: with
  `GROK_DEPLOYMENT_KEY` set, `GROK_MANAGED_CONFIG_URL` blackholed, no managed
  config on disk and `GROK_REMOTE_FETCH=0` — no socket with the switch off, two
  sockets with it on.
- `bf6a7dce` — tmux drive-through (`--no-leader --no-auto-update`, isolated
  `~/.grokdev`, `auth.json` temporarily moved aside): the welcome screen shows
  `No credential for model \`grok-4.5\`: set GROK_API_KEY …` and it stays for
  30 s; with the session restored, no line appears. Same commit: `auth_gate`
  6/6, `billing` 3/3, `share` 2/2, `cloud_gate` 2/2, pager remedy 8/8.
- `503900f0` — `xai-grok-tools`: 16 `image_gen`, 23 `video_gen` and 37
  `image_edit` + `media_endpoint` tests green; `xai-grok-voice`: 53 green;
  `xai-grok-shell`: 24 `prepare_*` tests green.
- `4764110f` — six previously failing tests now pass in one run; all six were
  stale assertions, none a regression.

### Phases 1-3 (quoted from commit messages, not re-measured)

All runs used `grok -p "Reply with exactly: OK"` on a debug binary, with xAI
blackholed via `10.255.255.1` (connect timeouts, not RST); the `8f6e0b09`
measurement additionally states an isolated profile.

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
  "xAI surfaces disabled" line emitted.

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

Test families named by the Phase 1-3 commits: `settings_fetch_budget`,
`remote_fetch`, `default_models`, pager `headless`, `is_xai_auth` (8),
`provider_identity` (2), `billing` (9).

## 5. What is not solved

- **The raw no-credential case.** With no key, no custom endpoint and a stale
  xAI model cache, the run no longer reaches the server: the authenticate
  preflight fails fast with the remedy in the message and a non-zero exit (see
  the hard table in section 3). Nothing synthesizes a local endpoint, and an
  unresolvable current model still reports nothing.
- **No local equivalent for the grok.com products.** Share links (the URL is
  built on grok.com), billing/usage, cloud sandboxes, the managed MCP gateway
  and managed config all still need an xAI account. They now refuse in one
  sentence instead of failing mid-request, but the capability is absent.
- **The new keys have no settings-registry entry.** `[search]`, `[tools.*]` and
  `[voice].api_base` resolve from the environment and the raw `config.toml`
  table only, so the TUI settings sheet does not list them and `/config` cannot
  edit them. The tables are absorbed as raw `toml` on the typed config (so they
  no longer trip `serde_ignored`), but `settings/defs.rs` and
  `settings/registry.rs` still name none of these keys. Deferred to Phase 5 by
  design.
- **`provider = "openai"` on video is a no-op alias of `auto`.** Only the image
  surfaces change wire on `openai`; `MediaProvider::is_openai_shape` is consulted
  by `image_gen` and `image_edit` only, and the OpenAI video API is not
  implemented. Setting it on `[tools.video_gen]` is harmless, not a translation
  layer.
- **No live third-party gateway run.** Every backend test runs against
  `wiremock`; the off-xAI battery blackholes xAI but talks to no real SearXNG,
  Tavily, Brave, vLLM or LiteLLM instance. The wire shapes are asserted, not
  end-to-end proven against a vendor.
- **Search quality is not comparable.** A `searxng` backend returns rendered
  hit lists; the `xai` backend returns the sampler's synthesis. The tool result
  shape is shared, the content is not.

### Known sharp edges

- `GROK_API_KEY` is written to `auth.json` (`xai::api_key`) by the authenticate
  step (`agent/mvp_agent/acp_agent.rs:770`) and the stored copy stays live:
  it is injected into `XAI_API_KEY` at startup when no env var is set
  (`acp_agent.rs:396-405`), and the provider vault returns it ahead of the env
  var (`agent/providers.rs:1672-1689`). Rotating the key means clearing that
  entry, not just re-exporting the variable.
- `grok agent headless` exits on stdin EOF when it serves over stdio: the stdin
  bridge shuts the ACP input down (`agent/app.rs:389-404`), so a harness that
  closes stdin ends the process.
- Loopback is first-party by default. Until `GROK_FIRST_PARTY_LOOPBACK=0` is
  set, `http://localhost:8000/v1` still receives `x-grok-*` headers and derives
  a first-party identity — which is correct for a local mock server and wrong
  for a real local gateway.
- `[search] api_key_env` names a variable, not a value. If that variable is
  unset, the backend has no key and reports it at call time; the factory warns
  once at resolution (`web_search/factory.rs:172-180`).
- `GROK_VOICE_BASE_URL` must be TLS. `http://localhost:8080` is rejected on
  purpose, because the bearer token would cross a cleartext socket.

### Follow-ups (committed)

The two workstreams an earlier revision of this document described as in flight
are committed and reflected above:

1. **`[xai] enabled` config twin** — `xai-grok-shell-base/src/util/mod.rs:127`
   (`resolve_xai_enabled_from`), `agent/config.rs:1279` (`XaiConfig`),
   `agent/init.rs:164`. Env still wins, and a blank/junk env value falls through
   to the config tier.
2. **`grok agent headless` without a grok.com session** — `agent/app.rs:624-631`
   makes the session optional and serves the agent over stdio ACP.

## 6. How to verify locally

### The committed battery

The battery is in the tree at
[`grok-off-xai-check.sh`](../grok-off-xai-check.sh). It builds nothing itself;
give it a fresh binary and it runs five scenarios with xAI blackholed, asserts
each one, and exits non-zero with a re-listed failure set if any assertion
regresses:

```sh
bash -c 'source ./grok-dev-env.sh && cargo build -p xai-grok-pager-bin'
bash ./grok-off-xai-check.sh                      # skips the session scenarios when no auth.json exists
bash ./grok-off-xai-check.sh --require-session    # ...or fails instead of skipping
OFF_XAI_KEEP=1 bash ./grok-off-xai-check.sh       # keep the scratch dir for inspection
```

Each scenario gets its own scratch `GROK_HOME` under `$TMPDIR`, so the battery
needs no hand-editing of `~/.grokdev` and leaves no state behind. Scenario 4 is
a control: it is the only one that asserts the relay *does* connect, which is
what stops scenario 3 from passing vacuously on a binary that never relays.

Two traps are worth knowing before you edit it, because both cost real time to
diagnose:

- **Do not write the stub gateway with a here-document.** `cat >file <<EOF`
  hangs under bash 5.3 — Homebrew's bash, which a bare `bash` resolves to
  whenever `/opt/homebrew/bin` is ahead of `/bin` in `PATH`. The heredoc arrives
  empty and `cat` blocks forever on the open pipe. macOS ships bash 3.2, where
  the identical heredoc is fine, so the failure is machine-dependent. The stub
  source is emitted with `printf` for that reason.
- **`--no-leader` belongs to the `agent` subcommand, not the top level.** The
  agent scenarios run `agent --no-leader headless`; putting a top-level
  `--no-leader` in front of `agent` aborts with
  `top-level --no-leader applies to the pager TUI, not the agent subcommand`.

The sections below are the manual equivalents, for when a scenario fails and
you want to see it by hand.

### Blackhole technique

`10.255.255.1:9` is a non-routable address with no listener: a TCP connect
there hangs until the client timeout instead of being refused with an RST.
That is what makes it a useful xAI stand-in — it exercises the timeout budget
rather than a fast failure.

Point both xAI hosts at it through their env overrides
(`agent/config.rs:571-575`) rather than editing `/etc/hosts` (an `[endpoints]`
key in `config.toml` would win over the env var, so check both if the run
still reaches xAI):

```sh
export GROK_CLI_CHAT_PROXY_BASE_URL='http://10.255.255.1:9/v1'
export GROK_XAI_API_BASE_URL='http://10.255.255.1:9/v1'
```

Then run the off-xAI recipe and check that the run neither stalls nor reports
an xAI error. Expected signals: exit 0, no `api.x.ai` in the network path, no
settings-fetch log line, and the `xAI surfaces disabled` line when
`GROK_XAI_ENABLED=0`. To confirm the relay specifically, seed a grok.com
session in `~/.grokdev/auth.json` and watch for the absence of a `grok.com`
socket (or of `relay_connecting` in stderr).

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
./grok-test.sh -p xai-grok-tools -- web_search media_endpoint
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

On the `-p` path those are top-level flags. On the `agent` subcommand
`--no-leader` is an *agent* option, so it goes after `agent` — a top-level
`--no-leader` in front of `agent` is rejected:

```sh
./target-dev/debug/xai-grok-pager agent --no-leader headless </dev/null
```

Headless calls can reach a configured gateway and consume credentials or
quota from the development profile — expect that, and do not point a
verification run at a production gateway by accident.