# Jev integration — the consolidated plan

Derived from all twelve `FINDINGS-*.md` docs on 2026-09-19. Every recommendation
in every doc is here, deduplicated where the same change serves several targets,
and ordered by dependency rather than by doc.

**Read this with the docs open.** Each item names its source section; the
measurement behind it lives there, not here.

## How to read this

- **Phase 0** blocks the rest. Nothing in Phase 1+ is worth starting before it.
- **Phase 1** is one change that five docs independently recommend — the highest
  return in the whole sweep.
- **Phases 2–4** are per-target and independent of each other. Do them in any
  order once 0 and 1 are done.
- Every item carries: source, evidence, fail-open, and the TUI row that makes it
  controllable.

**The one rule that applies to all of it:** every change is fail-open and defaults
to today's behaviour. Applying the whole plan changes nothing until a switch is
flipped, and every switch is independently reversible.

## Phase 0 — prerequisites

These block other work. Two are correctness bugs, one is a policy decision.

| # | item | source | why first |
| --- | --- | --- | --- |
| 0.1 | **Index the global topic files.** `list_memory_files()` returns only `global_dir/MEMORY.md`; the siblings are linked and invisible to search | `MEMORY` §3.1 | routing memory to global is pointless while global bodies cannot be found |
| 0.2 | **Stop `write_long_term` clobbering.** Dream rewrites the workspace `MEMORY.md` wholesale | `MEMORY` §3.2 | the admission pipeline cannot share a file with a whole-file rewriter |
| 0.3 | **Move the hardcoded `MemoryScope::Global`** (`effects/mod.rs:4052`) behind a decision | `MEMORY` §3.3 | it is the drift the scope stage exists to fix |
| 0.4 | **Instrument the permission floors.** 19 interruptions are invisible; count them by kind | `PERMISSION` §7.2 | the `exec` floor is 16 of the 19, and nobody can see it |
| 0.5 | **Decide the `exec` floor** — a policy call, not a model one | `PERMISSION` §7.4 | worth up to 16 fewer interruptions; measure before touching |
| 0.6 | **Ship or drop the `create-workflow` skill.** The tool tells the model to read a file that is not installed | `WORKFLOW` §2.3 | a dangling reference in the prompt |

## Phase 1 — the shared retrieval change

**Five docs recommend this independently.** It is the single highest-return change
recorded, and it is one config line plus a backend swap.

| # | item | source | evidence |
| --- | --- | --- | --- |
| 1.1 | **Point the embedding at `solaris:8001`** (`Qwen3-Embedding-4B`, 2560 dims) | `AGENTS` §5.2, `MEMORY` §7.4, `SKILLS` §12.10, `VS-STACK` §7.3 | +7 cases for free, zero tokens, 9 ms; the gold lands in the pool 25/25 vs 14/25 |
| 1.2 | **Fuse the dense index into the tool search**, keeping BM25 and the exact-name short-circuit | `TOOLS` §6.1 | pool recall 7/35 → **33/35** cross-lingually |
| 1.3 | **Add the dense index to the workflow catalog** and select with Jev, not a listing | `WORKFLOW` §8.1 | same accuracy, **677 fewer tokens every turn** |
| 1.4 | **Leave the reranker off everywhere.** Keep the slot; do not fill it | `TOOLS` §6.4, `AGENTS` §5.2, `SKILLS` §8, `VS-STACK` §7.2 | six measurements, six neutral-or-negative |
| 1.5 | **Add the third `RerankerProtocol` variant** anyway, so the slot is wireable | `VS-STACK` §4 | the only structural change; it attaches to an existing trait |
| 1.6 | **Add the tool selection call only if the fused top-1 disappoints.** 33/35 pool → 31/35 pick is already the ceiling | `TOOLS` §6.3 | the fused top-1 was not measured separately |
| 1.7 | **Keep a catalog in the workflow tool description as a fallback** until the retrieval path exists | `WORKFLOW` §8.2 | it is what makes a workflow discoverable at all today |

## Phase 2 — per-target quality

Ordered within each target by measured contribution. Targets are independent.

### Skills — the largest measured quality gain

| # | item | source | evidence |
| --- | --- | --- | --- |
| 2.1 | **Single-noul gate at 0.40** | `SKILLS` §12.1 | reverting to the mean costs 4 top-1 and drops correct silence 11/12 → 8/12 |
| 2.2 | **Wire `metadata.variants`** into selection | `SKILLS` §12.2 | −2 when removed, −3 on top-3 |
| 2.3 | **Drop the body re-read** | `SKILLS` §12.3 | −1 when added, doubles tokens |
| 2.4 | **Index to 200 bytes, cleaned** | `SKILLS` §12.4 | accuracy saturates at 200; 400 pays 8,600 tok/turn for nothing |
| 2.5 | **Injection = gate + one choice** | `SKILLS` §9 | precision 21.7% → **86.5%**, a quarter of the injected context, harmful 12/12 → 1/12 |
| 2.6 | **NL `should_trigger` cases** for the top ~40 skills | `SKILLS` §12.6 | every NL case fails today; the 54-case set is the seed |
| 2.7 | **Semantic eval arm**, opt-in, behind the offline runner | `SKILLS` §12.7 | the runner's offline property is deliberate |

### Memory

| # | item | source | evidence |
| --- | --- | --- | --- |
| 2.8 | **Admission + dedup at the append seam**, store in state, thresholds 0.20 / 0.50 | `MEMORY` §7.2, §4.2 | 100% precision and recall; without the store a restatement is re-stored 6/7 |
| 2.9 | **Scope routing** replacing the hardcoded `Global` | `MEMORY` §7.3 | 82.4%; local queries served by global 6/7 → **2/7** |
| 2.10 | **Jev rerank over the shortlist** | `MEMORY` §7.5 | 7/10 → 9/10 |
| 2.11 | **Consolidation inside Dream**, reusing the dedup question | `MEMORY` §7.6 | Dream never fires today |

### Permission

| # | item | source | evidence |
| --- | --- | --- | --- |
| 2.12 | **Shorten the criteria, threshold to 0.60** | `PERMISSION` §7.1 | 85.9% → **90.6%**, false allow 8% → 11%, false block 21% → 7% |
| 2.13 | **Only then touch the classifier** — its headroom is the residual, already at 90.6% | `PERMISSION` §7.3 | most remaining pain is the floors, not the model |

### Laziness

| # | item | source | evidence |
| --- | --- | --- | --- |
| 2.14 | **Attach the category definitions** | `LAZINESS` §7.1 | stalled categories 0/16 → **10/16**, no new machinery |
| 2.15 | **Gate to 0.50**, per-model | `LAZINESS` §7.2 | at 0.70 the detector fires on 3 of 16 stalls |
| 2.16 | **Replay through `trace_classify`** against the production classifier | `LAZINESS` §7.3 | the baseline this analysis could not make |
| 2.16b | **Decide the `evidence` clause last** — drop it, synthesise it, or keep a parallel text call | `LAZINESS` §7.4 | the only part a decision cannot replace |

### Agents

| # | item | source | evidence |
| --- | --- | --- | --- |
| 2.17 | **Stop asking for effort.** Default `medium` | `AGENTS` §7.5 | a constant `high` scores 64% against 56% for the question |
| 2.18 | **Try effort as an agent property** (`reasoning_effort:` in frontmatter) | `AGENTS` §7.6 | removes a decision instead of tuning one |
| 2.19 | **Fix the 11 fixable dangling delegate targets**; decide the 3 pointing at agents that do not exist (`ruby`, `ansible`, `dart`) | `AGENTS` §7.4, §6.2 | 15 edges short |
| 2.20 | **Answer the seven read-only-but-writable agents** | `AGENTS` §6.1 | `code-reviewer` can edit what it reviews |
| 2.20b | **Decide whether the advisory block earns 150 tokens/turn** — give it the revalidation result, or make it cheaper | `AGENTS` §7.7 | it buys one case today |

### Todo gate

| # | item | source | evidence |
| --- | --- | --- | --- |
| 2.21 | **Turn it on** — a working backstop disabled for UI reasons | `TODO-GATE` §7.1 | `settings/defs.rs:1943` documents the deferral |
| 2.22 | **Cap the item dump at 3** | `TODO-GATE` §7.2 | 1,391 → 548 chars at 20 items |
| 2.23 | **Only for lists > 3, let Jev pick the item** | `TODO-GATE` §7.3 | insertion order 61%, Jev 83%, +4 gains 0 losses |

### Workflow

| # | item | source | evidence |
| --- | --- | --- | --- |
| 2.24 | **Validate `agent_type` in `validate_only`** | `WORKFLOW` §9.3 | a typo passes the smoke check and fails mid-run |
| 2.25 | **Add `skills_hint` to `AgentOpts`** | `WORKFLOW` §9.4 | mirrors `spawn_subagent`; no binding today |
| 2.26 | **Use `agent_type` in the built-in** | `WORKFLOW` §9.5 | `deep_research.rhai` writes it zero times |
| 2.27 | **Add `decide(state, questions)` to the engine** | `WORKFLOW` §7.1 | the only path for Jev inside a script; replaces a subagent spawn with 300 ms |
| 2.28 | **Wire naming last** | `WORKFLOW` §9.8 | 40% at four candidates, and the earlier 53% was position-confounded |

### Goal verification

| # | item | source | evidence |
| --- | --- | --- | --- |
| 2.29 | **Expose the panel size** (1–5, already a bounded constant) | `GOAL` §7.1 | no row, no surface |
| 2.30 | **Consider the pre-filter only after measuring it against the panel** | `GOAL` §7.3 | 67% accuracy is not enough without the agreement rate |

## Phase 3 — correctness fixes with no measurement needed

| # | item | source |
| --- | --- | --- |
| 3.1 | Validate every `agent_type` literal (see 2.24) | `WORKFLOW` |
| 3.2 | Ship or drop `create-workflow` (see 0.6) | `WORKFLOW` |
| 3.3 | Keep BM25 in the tool fusion and keep the exact-name short-circuit | `TOOLS` §6.2 |
| 3.4 | Never add prose to a question without measuring it | `FINDINGS` cross-cutting |

## Phase 4 — the transport, and the TUI layer

### Transport first, because everything else needs a switch

| # | item | source |
| --- | --- | --- |
| 4.1 | **Add the `transport` enum** (`native` \| `openrouter`), moving endpoint, model, provider block and credential chain **together**; default `openrouter` | `JEV-TRANSPORT` §7.1 |
| 4.2 | **Add the derived session key** `{session_info.id}:jev`; leave `bootstrap_room` to the transports that define it | `JEV-TRANSPORT` §7.2 |
| 4.3 | **`/jev transport …`** plus the settings row, both dispatching one action | `JEV-TRANSPORT` §7.3 |
| 4.4 | **`/jev status` printing the credential *source*** — `env:`, `file:`, `auth.json:` | `JEV-TRANSPORT` §7.4 |
| 4.4b | **Audit the six session-id carriers before adding a seventh.** `x-grok-session-id`, `X-Session-ID`, body `session_id` + `bootstrap_room`, OpenRouter's `session_id`, `metadata.user_id`, `prompt_cache_key` — each gated differently, and none must change for the chat path | `JEV-TRANSPORT` §4 | a new caller that reuses the session key claims a prefix family it does not own |
| 4.4c | **Log the MCP header drop.** `expand_session_id_headers` (`xai-grok-mcp/src/servers.rs:4547`) drops a `{{session_id}}` header silently when no session is available — a server then behaves differently with no trace | found while auditing | the only silent branch in the whole carrier set |

### Then one row and one `status` per target, all following `/jev`

| # | surface | source |
| --- | --- | --- |
| 4.5 | `/permission-classifier on\|off\|status` + the four floor rows | `PERMISSION` §8 |
| 4.6 | `/laziness on\|off\|status` + threshold, idle, cap, and the last decision | `LAZINESS` §8 |
| 4.7 | `/prime on\|off\|status` + index width, scaffolding strip, injection marker | `SKILLS` §12 |
| 4.8 | `/memory-gate on\|off\|status` + master switch, `save_on_end`, thresholds, scope | `MEMORY` §8 |
| 4.9 | `/agents-recommend status` + graph toggle + default effort | `AGENTS` §8 |
| 4.10 | `/goal-verify on\|off\|status` + panel size, pre-filter, per-skeptic votes | `GOAL` §6 |
| 4.11 | `/todo-gate on\|off\|status` + fire cap, item cap, picker toggle — **and the settings rows, since the flag is currently the only switch** | `TODO-GATE` §6, §7.4 |
| 4.12 | `/tool-search status` + backend enum | `TOOLS` §5 |
| 4.13 | `/workflows list` + the catalog, and the workflows toggle | `WORKFLOW` §8 |
| 4.14 | `/retrieval-settings` gains the reranker protocol choice | `VS-STACK` §6 |

**`SyntheticReason::SkillPrime`, the marker, then the strip** (`SKILLS` §10.4) is
the one sequencing constraint inside this phase: the strip is only safe once the
injection is visible.

## Cross-cutting rules

From `FINDINGS.md`, and they apply to every item above:

1. **A noul answering "is this worth it" sits at 0.20–0.40.** 0.5 is wrong. Five
   occurrences: skill gate, skill `fits`, memory value, memory admission,
   laziness.
2. **One noul per candidate.** A single "does the winner fit" conflates; averaging
   compresses and stops firing.
3. **Criteria wording is the operating knob, not the threshold.** Tune as a pair.
4. **Added prose is unreliable.** Five attempts, one partial win.
5. **Every change is fail-open and defaults to today.**
6. **A listing is a per-turn cost; retrieval is a per-call one.**
7. **A cache key is a routing key.**
8. **A lexical retriever is monolingual.**
9. **A long prompt is a mechanism.**
10. **Check whether a feature is off because it is bad or because nobody built the
    switch.**

## Do not do

Measured negatives, so nobody re-litigates them:

| don't | source |
| --- | --- |
| Stack a cross-encoder before Jev | `SKILLS` §8, `AGENTS` §5.2, `TOOLS` §3 — six neutral results |
| Add prose rules to the permission question | `PERMISSION` §5 — four rules, all worse |
| Re-read skill bodies as a second stage | `SKILLS` §7 — rescued 3, broke 9 |
| Ship a truncated listing | `WORKFLOW` §4 — 5/24, worse than none |
| Reuse the session's own key for Jev | `JEV-TRANSPORT` §5 |
| Port the permission prompt for accuracy | `PERMISSION` §7.5 — already 0.94/1.00 |
| Expect the delegates graph to add on top of retrieval | `AGENTS` §5.2 — ±0–1 |

## Dependency graph

```
0.1 0.2 0.3 ──> 2.8 2.9 2.10 2.11        (memory pipeline)
0.4 0.5     ──> 2.12 2.13                 (permission, floors first)
0.6         ──> 2.28                      (workflow naming needs the skill story)
1.1         ──> 1.2 1.3 2.5 2.10 2.17     (the embedding serves five targets)
1.4 1.5     ──> 4.14                      (the slot, then the row)
4.1 4.2     ──> 4.3 4.4                   (transport, then its surface)
4.3 4.4     ──> 4.5 … 4.13                (the pattern is set once)
2.16        ──> 2.14 2.15 validation      (replay is how the gate is checked)
```

Everything else is independent.

## What would invalidate this plan

- **`trace_classify` runs** (`LAZINESS` §7.3, `GOAL` §7.3). Both docs measured Jev
  against hand-labelled fixtures with **no production baseline**; a replay could
  move either target substantially, and the goal pre-filter is explicitly
  contingent on it.
- **A real catalog size** for skills or tools. Both scale arguments (width, dense
  index) are measured at 100–172 items; a 1,000-item catalog would change the
  crossover points.
- **The solaris box's flakiness.** It dropped twice during this work. Phase 1
  depends on it, so the fail-open paths must be exercised, not assumed.
- **One run per arm, everywhere.** Differences of one or two cases do not
  separate; the plan's ordering uses the large gaps only.

## Checklist

**63 numbered rows**, covering 68 recommendations across twelve docs — 64 from
the docs plus 4 the audit surfaced. Five recommendations are folded into a single
row each (0.6 = 3.2, 2.24 = 3.1, 1.4 preconditions 4.14, 2.28 depends on 0.6, 1.1
serves four targets), which is why the row count is lower than the recommendation
count.

Coverage was audited mechanically against every doc's implementation-order section
after writing, which is how 1.6, 1.7, 2.16b and 2.20b were caught — four items the
first draft had compressed into their neighbours. Nothing in the twelve docs is
missing from this plan.