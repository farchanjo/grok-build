# Multi-Account Providers

You can run more than one instance of the same provider kind at once. This
guide explains what a provider **instance** is, how accounts are kept apart,
how account-qualified models appear in the picker, and what the safe lifecycle
operations look like. It is a boundary guide: everything below uses placeholder
identifiers, never real credentials.

See also [Custom Models](11-custom-models.md) for endpoint-level configuration,
[Configuration](05-configuration.md) for where these blocks live,
[Retrieval and Prime](30-retrieval-and-prime.md) for account-qualified
embedding/rerank references, and the provider references for
[OpenAI Platform](../providers/openai-platform.md) and
[OpenRouter](../providers/openrouter.md).

---

## Purpose and safety boundary

A provider **kind** (for example OpenAI or OpenRouter) is not an account. Grok
Build models a *kind* as the family of compatible wire protocols, while an
account is a concrete instance with its own credentials, catalog, and route
state.

- **Provider instance** — a named, stable configuration block under
  `[model_providers.<instance_id>]` with a declared kind and its own
  credential scope.
- **Stable ID and incarnation** — the instance ID identifies the configured
  block; an incarnation distinguishes a re-created instance from an older one
  so a stale refresh result cannot be applied to the wrong generation.
- **State** — an instance is `enabled` or `disabled`. A clean zero-reference
  removal forgets its live incarnation; a **forced** removal records an
  incarnation tombstone that blocks accidental reuse.
- **Route persistence** — the session remembers which instance and incarnation
  a model route used. A disabled or force-removed route never silently rebinds
  to a same-kind sibling.

Built-in compatibility instances (the ones `/providers` installs for xAI,
OpenAI, OpenRouter, Anthropic, and ChatGPT OAuth) remain **descriptors for the
built-in instance** — they are not extra accounts. Adding a second instance of
a kind is distinct from editing the built-in one.

---

## Account-qualified configuration

The following is placeholder-only TOML. The `env_key` values name the
environment variables that are expected to hold the keys; **no value is shown**,
and no key material belongs in `config.toml`.

```toml
# Two OpenAI Platform instances with distinct IDs and model IDs.
[model_providers.openai-platform-primary]
kind = "openai"
base_url = "https://api.example.com/v1"
env_key = "<OPENAI_PLATFORM_KEY_ENV>"
api_backend = "responses"

[model_providers.openai-platform-secondary]
kind = "openai"
base_url = "https://api.example.com/v1"
env_key = "<OPENAI_PLATFORM_SECONDARY_KEY_ENV>"
api_backend = "responses"

[model.openai-platform-primary-gpt-example]
model = "<MODEL_ID>"
model_provider = "openai-platform-primary"
name = "Primary platform example"

[model.openai-platform-secondary-gpt-example]
model = "<MODEL_ID>"
model_provider = "openai-platform-secondary"
name = "Secondary platform example"
```

```toml
# Two OpenRouter instances with distinct instance IDs and model IDs.
[model_providers.openrouter-team-a]
kind = "openrouter"
base_url = "https://api.example.com/v1"
env_key = "<OPENROUTER_TEAM_A_KEY_ENV>"
api_backend = "chat_completions"

[model_providers.openrouter-team-b]
kind = "openrouter"
base_url = "https://api.example.com/v1"
env_key = "<OPENROUTER_TEAM_B_KEY_ENV>"
api_backend = "chat_completions"

[model.openrouter-teama-provider-example]
model = "<MODEL_ID>"
model_provider = "openrouter-team-a"
name = "Team A example"

[model.openrouter-teamb-provider-example]
model = "<MODEL_ID>"
model_provider = "openrouter-team-b"
name = "Team B example"
```

Qualified model IDs look like `openai-platform-primary:<upstream-id>` and
`openrouter-team-a:<upstream-id>`: the configured instance ID is followed by a
colon and the complete upstream slug, so two accounts can advertise the same
slug without colliding. A slash may appear *inside* an OpenRouter upstream slug
(for example `vendor/model`); it is not the instance separator.

ChatGPT OAuth is a **separate route**: it is not the OpenAI Platform API-key
account. OAuth stores its own token and is selected only for routes whose base
URL is the Codex endpoint; it never supplies a credential for a Platform
API-key route.

Configured-instance credential resolution is **scoped to that instance**. If
an instance's own credential is absent it fails closed — Grok does not borrow
a sibling account's credential or fall back to an xAI session token.

---

## Catalog and model selection

- **Canonical / account-qualified IDs.** Every configured provider kind uses
  `instance:upstream`. This is the exact ID used by `/model`, `search_models`,
  session resume, and route persistence.
- **Duplicate upstream slugs.** Two accounts advertising the same upstream
  slug stay **separate candidates**. They are not deduplicated across
  accounts; each keeps its own canonical ID.
- **Picker qualification.** Where a bare slug would be ambiguous, the model
  picker qualifies entries with their account label so you can disambiguate.
- **Exact aliases** resolve to one canonical candidate; **unique aliases**
  resolve when exactly one instance exposes the slug; **ambiguous aliases**
  are not resolved silently; **gated aliases** identify entries withheld by
  the multi-account rollout switch; **missing aliases** fail closed; **shadowed
  entries** are surfaced rather than silently replaced.
- **Default resolution.** A defaulted config may reference the built-in
  model; when multiple instances advertise the same title, selection is
  qualified rather than guessed.

`search_models` returns the exact canonical selection IDs so you can hand
them back verbatim. Session resume keeps the previously resolved route even
across a hot reload that changes the next eligible selection.

---

## Credentials, cache, and failure isolation

`grok inspect` and `/providers` never expose credential variable names,
values, scopes, credential/cache fingerprints, or headers. Credential and
cache health use bounded status categories. `grok inspect --json` does include
the provider's full local lifecycle incarnation so automation can correlate
exact routes and tombstones; human output shortens it. An incarnation is a
random non-secret runtime identity, not an external account-owner identifier.

Safe credential statuses: `configured`, `environment`, `oauth`, `helper`,
`missing`, or `unavailable`. `environment` means a declared environment route
has a non-empty value in the current process; an unset declaration is
`missing`. `configured` is a **credential-status category**, not proof the
account is currently reachable.

Safe cache / catalog / capability states: `valid`, `mismatch`, `corrupt`,
`tombstoned`, `unavailable`, or `not_checked`. Grok reports the validity of a
record as observed; an unvalidated record is never labeled valid. On a failed
cache refresh, the last-known-good cached catalog remains usable.

Failure isolation is **account-local**: an HTTP 401 or 429 on one instance
does not invalidate a sibling's credential or catalog. Refresh and
authentication failures are bounded to the affected instance.

---

## Provider lifecycle

Select or list instances by stable ID:

```bash
grok provider list
grok inspect
grok -m openrouter-team-a:<MODEL_ID> -p "…"
```

`--provider <INSTANCE_ID>` targets a specific instance for operation surfaces
that support instance selection. `grok provider show <INSTANCE_ID>` and
`grok inspect` provide safe state and bounded reference-impact information.
Removal itself is a shared-state mutation and requires `--yes`; there is no
`provider remove --dry-run` flag.

```sh
# Clean removal succeeds only when reference impact permits it. It forgets the
# live incarnation but does not clear credentials or caches unless requested.
grok provider remove openrouter-team-b --yes

# Forced removal creates an incarnation tombstone and requires exact typed-ID
# confirmation. The optional clear flags remain independent and explicit.
grok provider remove openrouter-team-b --force \
  --typed-id openrouter-team-b --yes
```

Neither path silently remaps a persisted route to a same-kind sibling. Disable
keeps the recorded incarnation but blocks new use. Clean zero-reference removal
forgets the live row, and a later add mints a new incarnation. Forced removal
tombstones the prior incarnation so old bound sessions cannot rebind if the ID
is recreated; reselect explicitly after a lifecycle change.

---

## OpenAI Platform surface

See the [OpenAI Platform reference](../providers/openai-platform.md) for the
pinned full surface. Key points:

- The implementation requires the **complete pinned surface**, not an
  undocumented subset.
- Application and administration credentials remain structurally separate.
- Supported transports include HTTP JSON, SSE, multipart, and binary request
  bodies, plus the separate realtime WebSocket client.
- Mutating operations require confirmation, and `--dry-run` does not perform
  the operation.

---

## OpenRouter surface

See the [OpenRouter reference](../providers/openrouter.md) for the pinned
native baseline. Key points:

- **Native OpenRouter operations** (per-instance preferences, credits, and
  catalog calls) are OpenRouter-specific and not a full OpenAI administration
  clone.
- The OpenAI-compatible overlap is described separately.
- Supported transports include HTTP JSON, SSE, multipart, and binary
  behavior, with confirmation and `--dry-run` guarantees for mutating calls.

---

## Rollout and rollback

### Pre-rollout

1. Confirm Gate A–F evidence and the exact PR19/PR20/PR21 dependency revisions
   used by the release; keep the multi-account rollout decision consistent
   with Gate D.
2. Freeze representative compatibility fixtures for built-in xAI, OpenAI
   composite, OpenRouter, Anthropic, Z.ai, custom, no-auth, and auth-helper
   configurations, including aliases/defaults, legacy caches, session
   summaries/JSONL, retrieval/memory fixtures, and old/new leader frames.
3. Verify that no automatic migration deletes, renames, or copies credentials,
   and that legacy API-key scopes and built-in cache projections remain
   readable. Use temporary homes and local mocks only.
4. Review `grok inspect --json` and `/context` redaction fixtures before
   release; examples and output must contain placeholders and bounded status
   only.

### Phased rollout

1. Ship reader-first compatibility behavior before asking users to add
   instances.
2. Start with one configured instance and one retrieval profile in a
   non-production test environment. Verify exact canonical selection and
   lexical degradation while semantic services are unavailable.
3. Add duplicate-account and retrieval routes incrementally. After each
   change, verify `/model`, `search_models`, session resume, provider reference
   impact, `grok inspect`, and per-instance cache/credential status.
4. For memory, choose the named profile deliberately, approve the vector
   rebuild impact, and keep FTS available during rebuild or failure.
5. Monitor only bounded, non-secret health, degradation, generation, and local
   lifecycle-incarnation data. Diagnostics are not a credential or external
   account-owner identity oracle.

### Rollback

1. Stop new configuration edits and refreshes. Retain the current config,
   vault, lifecycle/cache state, session files, and memory files intact.
2. Revert only after confirming that the older binary reads retained legacy
   built-in scopes and caches. New-only account-qualified selections may become
   **unavailable**; they must fail `missing`/`unavailable`, never rebind to a
   sibling.
3. Retain the newer binary's last-known-good cache/state for forward recovery.
   Do not hand-edit UUID/incarnation, binding, cache, session-route, or vector
   fingerprint metadata.
4. If a named memory profile or vector migration caused the issue, leave an
   incomplete index in its FTS-only-safe state or use the explicit recovery or
   rebuild path; never label old vectors compatible with a new source.
5. On re-upgrade, rerun inspect/context redaction and exact-route checks before
   resuming refreshes. Do not delete, copy, rename, hand-edit, or migrate
   credentials, caches, aliases, tombstones, or route metadata as rollback;
   destructive cleanup is a separately approved migration.

---

## Privacy and external-data disclosure

Provider requests, catalog calls, and account-specific model routes may
disclose request data to the **selected** external provider under that
provider's policy. `grok inspect` and session diagnostics expose only bounded
status, generation, count, and outcome data — never credentials, headers,
endpoint values, or raw provider bodies.
