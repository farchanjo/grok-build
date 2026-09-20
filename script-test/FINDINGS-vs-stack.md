# Jev against the retrieval stack — findings

Consolidated 2026-09-19. The oldest doc here, written before the `FINDINGS-*.md`
pattern settled; brought in line with it. Companion to `FINDINGS-SKILLS.md`, which
now carries the full retrieval factorial this doc's head-to-head predates.

Measured against `jev-1.13`, native transport, live dev profile read-only. No
Rust was changed.

## 1. Goal

Find out whether the retrieval machinery the profile already declares can carry a
decision layer, and where exactly such a layer would attach. This is the
infrastructure question behind `FINDINGS-SKILLS.md`.

## 2. What exists today (verified)

Read from `~/.grokdev/config.toml`:

- `[embedding_models.prime-local]` — `openai/text-embedding-3-small`, 1536 dims,
  through the `prime-embeddings` provider (`openrouter.ai/api/v1`).
- `[retrieval_profiles.prime-main]` — that embedding model, `max_candidates = 50`,
  `max_results = 10`, `fallback_strategy = "deterministic"`, and
  **`reranker_models = []`**.
- `[prime.skills] enabled = true` (deadline 3000 ms, degrade on error) and
  `[prime.agents] enabled = true` — the pipeline is live in the dev profile.
- `[memory] retrieval_profile = "prime-main"`, `vector_store = "milvus-vm"`.
- `[vector_stores.milvus-vm]` — `vm.services:19530`, verified reachable, with
  `grok_mem_ce8c9272b11ef26c` live and the fingerprint scheme `mirror.rs`
  documents.

So embeddings are configured and working, Milvus is configured and up, and the
**reranker slot is declared and empty**. That empty slot is the integration point.

## 3. Measured: the head-to-head

Arm A reproduces the selector's core (`prime/skills.rs`: deterministic evidence,
FTS + KNN over indexed metadata, weighted RRF). It is a faithful core, not the
exact implementation — the real one also scores `when-to-use` / path evidence and
caps flooding. Superseded by `FINDINGS-SKILLS.md` §8, kept here for the
integration argument it supported.

| arm | top-1 | gold in top-10 | p50 | tokens |
| --- | --- | --- | --- | --- |
| **A** BM25 + dense + RRF | 42.6% | **90.7%** | <1 ms | 0 |
| **B** Jev ranking the roster wide | 72.2% | — | 732 ms | ~200k |
| **C** A's shortlist, Jev reranks | **77.8%** | 90.7% | 704 ms | ~200k |

C against A: 20 recovered, 1 broken. Ceiling for C was 49/54 (90.7%).

1. **The stack is an excellent retriever and a poor selector.** 90.7% recall@10
   against 42.6% top-1 — it finds the answer in the top ten for nine queries in
   ten and cannot order them.
2. **Jev in the reranker slot is +35 points, with nothing removed.**
3. **Jev alone is worse than the hybrid** (72.2% vs 77.8%), so the retriever earns
   its place. Not a replacement argument.
4. **Latency is the price**: ~700 ms against `deadline_ms = 3000` and
   `degrade_on_error = true` already set. It fits.

## 4. The integration point, precisely

The reranker is a trait, not a special case:

```rust
async fn rerank(&self, query, documents, top_n, cancel) -> RetrievalResult<RerankResult>
```

with `RerankerProtocol { OpenaiCompatible, CohereCompatible }` as the only two
wire shapes. Jev's shape is neither: it posts `{model, state, questions}` and
answers `{answers: {id: {type, noul}}}`, while an OpenAI-compatible rerank posts
`{model, query, documents, top_n}` and answers `{results: [{index, relevance_score}]}`.

So integration is one of:

- **a third `RerankerProtocol` variant** that renders one `noul` question per
  candidate and maps the answers back to `RerankHit`s. Fits the existing bounds,
  deadlines, fallback order and telemetry with no new plumbing.
- or a separate stage beside the reranker, which duplicates what the profile
  already declares.

The first removes nothing and adds no parallel machinery.

## 5. Design

| stage | attachment | fail-open |
| --- | --- | --- |
| rerank | third `RerankerProtocol` variant, over the shortlist | RRF order |
| bounds | inherit `max_candidates`, `max_results`, `deadline_ms` | unchanged |
| provider | `RerankerProtocol` already carries a provider config | unchanged |

Note for whoever wires it: no reranker is reachable today (`llm.flashnext…/v1/rerank`
→ 530, `vm.services:8080|8000/rerank` → no listener), so there is no incumbent to
beat. The slot is empty in practice, not just in config. `FINDINGS-SKILLS.md` §8
later measured the axis and found the reranker neutral-to-negative — so the
recommendation is to wire the slot and leave it off.

## 6. TUI configuration

**What exists.** `reranker_models` is edited from `/retrieval-settings`
(`retrieval_settings_modal.rs`), which also covers embeddings, retrieval
profiles, prime and memory — a complete surface for adding a reranker entry.

**What is missing.** A third protocol choice in the model picker, and a
`/rerank on|off` toggle.

**Shape, following `/jev`.** `/jev on|off|status` dispatches a typed action, the
same one the settings row dispatches; the value persists and the live session
picks it up through the config reload fan-out.

## 7. Implementation order

1. **Add the third `RerankerProtocol` variant** — the only structural change.
2. **Leave it off** until `FINDINGS-SKILLS.md` §8 says otherwise; the axis measured
   neutral-to-negative.
3. **Point the embedding at `solaris:8001`** — measured +7 cases for free in
   `FINDINGS-AGENTS.md` §5.2, and the biggest single retrieval win recorded.

## 8. Hazards

- **The retriever's recall is the reranker's ceiling.** At 90.7% pool recall no
  reranker gets past it, so measure recall before blaming the reranker.
- **The protocol shape differs.** Mapping Jev's answers to `RerankHit` needs an
  index, and the answers come back keyed by question id — name the questions by
  index, not by name, or two candidates with the same name collide.
- **`fallback_strategy = "deterministic"` is already set**, so a Jev failure
  degrades to RRF order without new code — do not add a second fallback.

## 9. Not verified

- Arm A is a core reproduction, not the production code path; 42.6% is a proxy.
- One run per arm; superseded measurements carry the same caveat.
- Milvus was verified reachable and later exercised over REST
  (`FINDINGS-AGENTS.md`), but not through the Rust mirror.
- The `RerankerProtocol` trait and its two variants were read, never implemented
  against.
- Nothing was measured inside the TUI.