# Wiring Jev's transport, and the session-id hazard — findings

Verified 2026-09-19 against `jev-1.13`, live dev profile read-only. No Rust
changed. Companion to the other `FINDINGS-*.md`; `FINDINGS.md` is the umbrella.

## 1. Goal

Let the user pick Jev's transport — the native TypeSafe API or OpenRouter's alpha
decision endpoint — **from the interface**, without a config edit and without
breaking the session.

## 2. What exists today (verified)

| piece | where | behaviour |
| --- | --- | --- |
| endpoint | `[compaction.jev] endpoint` | default `https://openrouter.ai/api/alpha/decisions` |
| model | `[compaction.jev] model` | default `~typesafe/jev-latest` |
| credential | `api_key_env` → `GROK_JEV_API_KEY` → `auth.json` OpenRouter scope | resolved in `jev_prune/client.rs` |
| provider block | `[compaction.jev] provider` | zdr / data_collection / require_parameters, OpenRouter-only |
| toggle | `/jev on\|off\|status` | dispatches `Action::SetCompactionJevEnabled` |
| settings row | `compaction_jev_enabled` | the same action, so the two cannot drift |

So the toggle exists and the transport is three config fields with **no surface**.

## 3. The transport must move as a unit

Both transports were probed and both work — same model behind, identical answers,
`choice` and `score` accepted on both. But the defaults are **not
interchangeable**:

| | native | OpenRouter |
| --- | --- | --- |
| endpoint | `https://api.typesafe.ai/v1/systemone` | `https://openrouter.ai/api/alpha/decisions` |
| model | `jev-latest` | `~typesafe/jev-latest` |
| provider block | rejected | accepted |
| credential | `TYPESAFE_API_KEY` / `TYPESAFE_AI_FABRICIO_KEY` in `~/.llm-key` | OpenRouter key in `auth.json` |

**Setting one without the others fails confusingly**: the native endpoint with the
`~typesafe/` prefix 404s, and the OpenRouter endpoint with a bare `jev-latest`
fails model resolution. The `provider` block sent to native is an unknown field.

So the interface should expose **one enum**, not three fields:

```
[jev] transport = "native" | "openrouter"
      # sets endpoint, model, provider and the credential chain together
      # individual overrides stay available for the unusual case
```

## 4. The session id: today there is none, and that is deliberate

**The Jev client sends no session id.** Its body is `{model, state, questions}`
plus an optional `provider`; the client's own doc says the credential comes "from
the profile (OpenRouter scope in `auth.json`), never from the session".

That matters because the main inference path **does** carry one, and it is
load-bearing. `session_id: Some(self.session_info.id.to_string())` is stamped
per turn (`inference_turn.rs:867`) and is the single source of:

| provider | carrier |
| --- | --- |
| first-party | `x-grok-session-id` header (`client.rs:82`), **only when `first_party`** |
| self-hosted OpenAI-compatible | `X-Session-ID` header (`client.rs:1495`) |
| SGLang / vLLM | `session_id` **and** `bootstrap_room` in the body (`client.rs:2097`) |
| OpenRouter | native `session_id` field |
| Anthropic | `metadata.user_id` |
| OpenAI | `prompt_cache_key` |

Two facts that decide the design:

1. **The `x-grok-*` headers never reach OpenRouter** — `GrokRequestHeaders::apply`
   returns early when `!first_party`. So on the OpenRouter transport the main path
   sends no `x-grok-session-id` either; OpenRouter reads the body's `session_id`.
2. **`bootstrap_room` is `fnv1a_32(key)`** (`client.rs:2098`). It is a *routing*
   key, not decoration: two request streams that share it are assumed to share
   prefixes.

## 5. The hazard, precisely

If a session id is added to the Jev path, the safe choice is **a derived key, not
the session's own key**:

```
session_id: "{session_info.id}:jev"
```

Why not reuse `self.session_info.id`:

- **The payload shapes differ.** Jev posts `{state, questions}` — no messages, no
  tools, no system prompt. Sharing `bootstrap_room` with the chat stream tells a
  router that both belong to the same prefix family, which is false. At best the
  cache never hits; at worst a router pins them together and thrashes.
- **A separate lane costs nothing and breaks nothing.** The chat path's key is
  untouched, so nothing about the main flow can regress — which is what "cuidado
  para não quebrar tudo" asks for.
- **It still buys affinity within Jev's own traffic**, which is the point: repeated
  Jev calls in one session carry near-identical states, so they can share a cache
  lane.

What to avoid:

- **Do not invent a random id per call.** A fresh key per request gets no affinity
  at all and makes the field decorative.
- **Do not send `bootstrap_room` to the native endpoint** without checking — it is
  an SGLang/vLLM convention, and native TypeSafe may ignore or reject it.
- **Do not derive the key from anything unstable** (turn index, token counts), or
  the lane changes every turn.

## 6. TUI configuration

**What exists.** `/jev on|off|status` and the `compaction_jev_enabled` row, both
dispatching the same typed action.

**What is missing.** The transport, the credential source, and any visibility of
which endpoint is live.

**Shape, following `/jev`.** One typed action, one slash command, one settings
row, one `status`:

```
/jev transport native|openrouter   -> Action::SetJevTransport(Transport)
/jev status                        -> effective transport, endpoint, model,
                                      credential source (never the key), and
                                      whether the last call succeeded

settings row: "Jev transport" enum {native, openrouter}, default openrouter
              (today's behaviour), beside the existing compaction_jev_enabled row
```

`status` printing the credential **source** (`env:GROK_JEV_API_KEY`,
`file:.llm-key:TYPESAFE_AI_FABRICIO_KEY`, `auth.json:openrouter`) is the piece
that makes a misconfiguration diagnosable: today a wrong key fails as a 401 with
no indication of which chain was tried.

## 7. Implementation order

1. **Add the `transport` enum** with both endpoints and credential chains wired,
   defaulting to `openrouter` so nothing changes for anyone.
2. **Add the derived session key** (`{id}:jev`) as a body field on both transports,
   behind the same enum, and leave `bootstrap_room` to the transports that define
   it.
3. **`/jev transport …` and the settings row**, both dispatching one action.
4. **`/jev status` with the credential source.**
5. Keep the three individual overrides for the unusual case.

## 8. Hazards

- **Endpoint and model are a pair.** The enum exists to stop the mismatch; do not
  re-expose them as free fields without a warning.
- **The provider block is OpenRouter-only.** Sending it to native is an unknown
  field; if native ever tightens validation this becomes a 422.
- **A 401 must name the chain tried**, or the two-transport setup is undebuggable.
- **`first_party` gates the `x-grok-*` headers.** Anything that assumes those
  headers on a third-party transport is wrong, and that includes the Jev path.

## 9. Not verified

- Both transports were probed and answered; the native one was exercised with
  `TYPESAFE_AI_FABRICIO_KEY` from `~/.llm-key`, which is a name the SDK does not
  use (`TYPESAFE_API_KEY`) — the chain must accept both.
- `session_id` and `bootstrap_room` were read in the client, not observed on the
  wire; whether OpenRouter or native act on them for Jev's payload shape is
  unmeasured.
- The `provider` block being rejected by native is inferred from it being an
  OpenRouter extension, not tested against the native endpoint.
- Nothing was measured inside the TUI.