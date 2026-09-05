# Retrieval and Prime

*Retrieval* is how Grok Build gathers relevant context (documents, memory,
skills) before a turn, and *prime* is the bounded injection of that gathered
context into the conversation. This guide explains named retrieval profiles,
typed embedding and rerank protocols, ordered fallback and deadline
enforcement, prime budgets, the memory boundary, and the optional remote
vector-store mirror.

See also [Configuration](05-configuration.md), [Memory](13-memory.md),
[Multi-Account Providers](29-multi-account-providers.md), and the provider
references for [OpenRouter](../providers/openrouter.md) and
[OpenAI Platform](../providers/openai-platform.md).

---

## What retrieval and prime do

Grok Build models retrieval as a **profile graph**: a named selection of an
embedding model, an optional reranker, and ordered routes. A retrieval profile
binds embeddings and (optionally) reranking to concrete provider/model/protocol
references.

Grok distinguishes the **disk/config candidate** (what is currently written to
config) from the **published runtime snapshot** (the graph the session is
actually using). When a candidate is invalid, the profile retains the
**last-known-good published graph** and reports a warning instead of silently
publishing a partial graph.

---

## Profile and graph configuration

Named embedding and reranker entries bind a safe model/protocol reference to an
exact provider instance. A profile then lists those route IDs in deterministic
fallback order and applies one strict operation-wide budget.

```toml
# Provider instances are credential-scoped separately from this graph.
[model_providers.provider-primary]
kind = "openai_compatible"
base_url = "https://embedding-primary.example.test/v1"
env_key = "<PRIMARY_EMBEDDING_KEY_ENV>"
capability_mode = "manual"
[model_providers.provider-primary.capabilities]
embeddings = true

[model_providers.provider-fallback]
kind = "openai_compatible"
base_url = "https://embedding-fallback.example.test/v1"
env_key = "<FALLBACK_EMBEDDING_KEY_ENV>"
capability_mode = "manual"
[model_providers.provider-fallback.capabilities]
embeddings = true

[model_providers.provider-reranker]
kind = "openai_compatible"
base_url = "https://reranker.example.test/v1"
env_key = "<RERANKER_KEY_ENV>"
capability_mode = "manual"
[model_providers.provider-reranker.capabilities]
rerank = true

[embedding_models.example-primary]
provider = "provider-primary"
model = "<EMBEDDING_MODEL>"
protocol = "openai_compatible"
dimensions = 1536
batch_size = 32
max_input_tokens = 8192

[embedding_models.example-fallback]
provider = "provider-fallback"
model = "<EMBEDDING_MODEL_FALLBACK>"
protocol = "openai_compatible"

[reranker_models.example-reranker]
provider = "provider-reranker"
model = "<RERANKER_MODEL>"
protocol = "cohere_compatible"
endpoint = "rerank" # relative path only; never an origin override

[retrieval_profiles.example]
embedding_models = ["example-primary", "example-fallback"]
reranker_models = ["example-reranker"]
fallback_strategy = "deterministic"
max_candidates = 12
max_results = 6
deadline_ms = 10000 # optional override; default is 10000
max_attempts = 3
max_input_tokens = 8192
max_output_tokens = 4096

[prime.skills]
enabled = true
retrieval_profile = "example"
max_results = 3
max_body_chars = 2000
max_total_chars = 6000
max_tokens = 1500
max_context_fraction = 0.05
deadline_ms = 3000
degrade_on_error = true

[prime.agents]
enabled = true
retrieval_profile = "example"
max_results = 3
max_total_chars = 6000
max_tokens = 1500
deadline_ms = 3000
degrade_on_error = true

[memory]
retrieval_profile = "example"
```

There is no separate `lexical_only` configuration table. When semantic routes
are unavailable, consumers that support degradation continue with their
lexical/full-text path and report the safe degradation category. Route order
is deterministic and recorded. Diagnostics expose only bounded generation and
fingerprint values — never retrieved content, vectors, model input, or raw
service error text.

---

## Protocol boundaries

- **Typed OpenAI-compatible embeddings.** Embeddings are typed to the
  OpenAI-compatible embeddings protocol.
- **Typed rerank protocols** are distinct from nonstandard/custom rerank
  protocols. An arbitrary OpenAI-compatible endpoint is **not** assumed to be
  a typed reranker; an endpoint only participates as a reranker when it
  actually implements the typed rerank protocol.

### Solaris no-auth configuration evidence

Solaris references here are **configuration evidence only**. Grok Build does
not start, configure, or administer Solaris services in any way.

```toml
[model_providers.solaris-retrieval]
kind = "openai_compatible"
base_url = "https://solaris-embedding.example.test/v1"
auth_scheme = "none"
catalog_enabled = false
capability_mode = "manual"

[model_providers.solaris-retrieval.capabilities]
embeddings = true
rerank = true

[embedding_models.solaris-embedding]
provider = "solaris-retrieval"
model = "<EMBEDDING_MODEL>"
protocol = "openai_compatible"

[reranker_models.solaris-reranker]
provider = "solaris-retrieval"
model = "<RERANKER_MODEL>"
protocol = "cohere_compatible"
endpoint = "rerank"

[retrieval_profiles.solaris]
embedding_models = ["solaris-embedding"]
reranker_models = ["solaris-reranker"]
deadline_ms = 30000
```

The `solaris-embedding.example.test` hostname is illustrative. It is contacted
only if a user deliberately installs and selects this placeholder-derived
configuration; no documentation test or normal default contacts it.

---

## Degradation and deadlines

- **Ordered fallback.** Routes are tried in the configured order.
- **Deadline enforcement.** A configured deadline bounds the whole operation;
  exceeding it degrades rather than blocking the turn.
- **Disabled/missing profile.** A profile that is disabled or missing reports
  `service_disabled` or `profile_missing`.
- **Semantic-unavailable and rerank-unavailable** label the specific
  degradation.
- **Budget exhaustion** labels the case where the injection budget was capped.
- **Lexical-only operation** surfaces when no semantic service is available:
  Grok still operates on lexical/full-text matching.

A partial invalid graph is **never** silently published as the usable graph;
the session keeps the last-known-good snapshot until a complete valid
candidate is available.

---

## Prime eligibility and privacy

- **Native-only.** Prime runs for native Grok sessions; synthetic or external
  turns do not prime.
- **Real-user-turn gate.** Prime runs only for an eligible real native user
  turn.
- **Metadata-only skill selection.** Skill selection uses metadata (name,
  description) only; skill bodies are loaded only after a bounded selection
  and are not exposed through `inspect`/`/context` diagnostics.
- **Advisory callable-agent recommendations.** Recommendations are names only —
  no spawn implication, descriptions, scores, sources, or bodies. A
  recommended agent is never spawned automatically. The session's CLI-agent,
  toggle, and global subagent gates are spawn-time policy; the current agent
  definition and plugin registry remain live. The live spawn gate remains
  authoritative before any task starts.
- **Explicit slash pin.** A user slash selection (for example pinning a skill
  via `/<skill>`) is respected; pinning limits which skill is used.
- **Hidden bounded reminder.** Prime may send the model a hidden reminder
  containing bounded, budget-truncated snippets from selected skill bodies.
  Unselected bodies are not included. The wrapper adds no credentials, but
  sensitive text stored inside a selected skill becomes model-visible; do not
  put secrets in skill bodies.

### Enabling prime (configuration requirements)

Prime is **disabled by default** and fails closed when misconfigured — a
missing or invalid configuration never turns prime on partially, and the
runtime does not log a turn-level error. To enable it in `config.toml`:

1. Define a provider instance (`[model_providers.<id>]`, `openai_compatible`),
   an embedding model that references it (`[embedding_models.<id>]`), and a
   retrieval profile that lists at least that embedding model id
   (`[retrieval_profiles.<id>]`).
2. Point each consumer at the profile:
   `[prime.skills] enabled = true` + `retrieval_profile = "<profile-id>"`, and
   likewise for `[prime.agents]`.

An enabled consumer without a `retrieval_profile`, a profile that references
no embedding model, or a dangling provider reference makes the whole graph
invalid; the registry then keeps its disabled snapshot and prime stays off.
The authoritative status is the **Retrieval & Prime** section of the `inspect`
output (`validity`, `enabled`, `prime: skills.enabled=… agents.enabled=…`),
not any turn log.

Two further behaviors matter in practice:

- **Semantic vs deterministic.** The semantic refinement calls the profile's
  embedding provider per eligible turn. With `fallback_strategy =
  "deterministic"` and an unreachable provider, the run degrades
  (`degrade_on_error` defaults to `true`) and selection falls back to the
  deterministic ranking — useful for local validation without a live endpoint.
- **Body containment.** Selected skill bodies load only from the session
  workspace (cwd and git root) or from the Grok home — which is where bundled
  and user-scope skills live. Skills elsewhere on disk are ranked but dropped
  at load time (`NotContained`).

---

## Budgets and diagnostics

Final **UTF-8-safe** injected character/token accounting is reported: the
numbers you see are after truncation and budget enforcement, not the requested
candidate sizes. Configured caps are available in the session-info JSON
snapshot; the human `/context` and `/session-info` views show final injected
usage without claiming the context-fraction-adjusted effective cap.

Synthetic/external turns do **not** overwrite the latest real native turn's
prime result. A disabled outcome remains available in ACP/session JSON, while
the empty default state is omitted from human `/context` and `/session-info`
output. Existing token rows are preserved; prime display is additive and is not
a second token category.

---

## Memory boundary

- **Named profile precedence.** A named retrieval profile takes precedence
  over legacy memory configuration; legacy configuration synthesizes a
  compatible profile where applicable.
- **Pinned embedding space.** The persistent memory backend pins its first
  usable embedding space/profile at index open. A reload changes the **next**
  eligible prime turn, not the active vectors.
- **Changed embedding identity** requires the existing serialized safe rebuild
  or a new session. During an incomplete or failed rebuild, memory remains
  FTS-only and does not claim vector compatibility.
- **Rerank placement.** Reranking happens after retrieval, according to the
  selected profile.
- **Rollback-safe warning.** Reverting does not rewrite active vectors; there
  is no destructive cleanup step.
- **Memory Modes (`local` vs `milvus`).** `[memory] mode` chooses between
  `"local"` (the default: local SQLite index with sqlite-vec and FTS5, zero
  Milvus involvement) and `"milvus"` (hard-remote primary store: Milvus BM25
  keyword search and dense KNN, local SQLite for chunk bookkeeping only).
  Configurable per-workspace in `.grok/config.toml` (gated by folder trust).
- **Remote vector-store mirror.** `[memory] vector_store` points to a named
  `[vector_stores.<id>]` entry (required in `milvus` mode). In `local` mode,
  leaving it unset keeps memory entirely local.

---

## Inspect, context, and session information

Safe profiles, generation, status, names, and counts are disclosed through
[slash commands](04-slash-commands.md) and the
[Configuration](05-configuration.md) guide. The safe summary never contains
prompts, bodies, vectors, raw errors, endpoints, or credentials.

---

## External-data disclosure and operational examples

Embeddings and reranking send the data required by the configured operation to
the selected external provider. This guide uses placeholders and local
fixtures only; Solaris is configuration evidence only and is not administered
by Grok Build.

---

## Rollout and rollback checklist

Use the full pre-rollout, phased-rollout, and rollback procedure in
[Multi-Account Providers](29-multi-account-providers.md). In particular:

- Confirm Gate A–F evidence and the PR19/PR20/PR21 dependency revisions.
- Freeze the full provider, alias/default, legacy-cache, session/JSONL,
  retrieval/memory, and old/new leader fixture matrix—not only embedding and
  reranker models—and review `grok inspect --json` plus `/context` redaction.
- Verify that no migration deletes, renames, or copies a credential and that
  legacy API-key scopes/cache projections remain readable.
- Start with one exact provider instance/profile in a non-production test
  environment. Validate canonical route selection and lexical degradation
  first, then add semantic and duplicate-account routes incrementally.
- After each route change, verify `/model`, `search_models`, session resume,
  provider reference impact, inspect, and per-instance cache/credential state.
- Before enabling memory vectors, approve the disclosed rebuild impact. Keep
  FTS available during every rebuild or failure.
- On rollback, stop edits/refreshes and retain config, vault, lifecycle/cache,
  session, memory, and last-known-good state. Never hand-edit identity, binding,
  route, or vector-fingerprint metadata.
- On re-upgrade, rerun inspect/context redaction and exact-route checks before
  resuming refreshes. Monitor only bounded non-secret health, degradation, and
  generation data.

---

## Prime index operations and local-only fallback

Prime metadata is stored separately from Memory. Status, missing-only
backfill, full rebuild, and cancel are generation-aware and secret-free.
Saving a retrieval profile is not a rebuild. When semantic routes are
unavailable, Prime and Smart search keep the exact deterministic local order
and report a bounded degradation label.

Configured-profile embedding runs only after explicit confirmation. The UI
shows the profile id, never an endpoint. See
[Strict Skills Migration](31-strict-skills-migration.md#index-operations).

## Remote vector-store mirror

SQLite stays the authoritative vector store for Memory and the Prime
metadata index. A named `[vector_stores.<id>]` entry describes an optional
remote **serving mirror**: Grok writes vectors to SQLite first, then fans
the change out to the mirror best-effort. Reads go mirror-first only when
the mirror is verified ready — the collection fingerprint, dimensions, and
row count agree with SQLite — and any mismatch, error, or unreachable
server falls back to the sqlite-vec path with identical result semantics.
Mirror operations never block a turn or fail a session.

Resync is self-healing. A stale, empty, or newly reachable mirror is
repopulated by streaming the stored vectors from the SQLite vec tables in
idempotent batches, verified by row count and fingerprint tag. No
re-embedding happens and no data is lost; during a resync window, reads
continue from SQLite, so there is no serving gap. The default is
sqlite-only: with no store selected, no remote server is contacted.

### Configuration

```toml
[vector_stores.local-milvus]
backend = "milvus" # currently the only supported backend
uri = "http://localhost:19530"
timeout_secs = 10  # optional per-call timeout; default 10, minimum 1

[memory]
vector_store = "local-milvus" # mirror the memory index (optional)

[prime]
vector_store = "local-milvus" # mirror the prime metadata index (optional)
```

- `[vector_stores.<id>]` declares the store. `backend` currently accepts
  only `"milvus"`; `uri` must start with `http://` or `https://`; the
  optional `timeout_secs` bounds each mirror call (default `10`, floored at
  `1`). Only non-secret fields live in config; a token key is rejected.
- `[memory] vector_store` selects the store for the memory index, and
  `[prime] vector_store` selects the store shared by the `skills` and
  `callable_agents` metadata collections. Both keys are optional siblings of
  `retrieval_profile`; the `[vector_stores.*]` table and both selection keys
  are stripped from untrusted configuration patches, so a patch can neither
  define a store nor redirect a selection.
- An unset selection key keeps that consumer sqlite-only.

### Credentials

The Milvus bearer token never lives in config. Grok resolves it in order:

1. The `MILVUS_TOKEN_FOR_<ID>` environment variable, where `<ID>` is the
   store id uppercased with non-alphanumeric characters replaced by
   underscores (`local-milvus` becomes `MILVUS_TOKEN_FOR_LOCAL_MILVUS`).
2. The application credential file (`auth.json` under the Grok home) at the
   vault scope `milvus::<store-id>::token`, read through the same
   provider-secret chain as model-provider credentials.

The token is never logged and is passed only to the store client.

### Security note

Enabling a Milvus store sends memory text to that server. Memory chunk text
and the vectors derived from it are transmitted to and stored on the
configured Milvus server (the prime collections mirror metadata vectors the
same way). Treat the server as trusted as the Grok home itself. Inspect,
`/context`, and `/session-info` disclose only bounded, non-secret mirror
state — backend, state, and row counts — never vectors or tokens.

### Troubleshooting

- **Server unreachable.** Search keeps working: memory and prime vector
  reads fall back to SQLite, the mirror is marked unavailable, and the
  failure is logged once rather than per query.
- **Server reachable again.** The mirror resyncs from the stored SQLite
  vectors automatically; no reindex or re-embed step is required.

---

## Privacy

Retrieval and prime accept only bounded, non-secret context. Inspect,
`/context`, and `/session-info` report categories, counts, and states — never
retrieved bodies, prompts, vectors, raw provider errors, or credentials.
Memory and Prime databases remain isolated. Enabling a remote vector store
additionally sends the mirrored memory text and its derived vectors to the
configured server. Rollback retains last-known-good
state and never treats quarantine as repaired. See
[Strict Skills Migration](31-strict-skills-migration.md#privacy).
