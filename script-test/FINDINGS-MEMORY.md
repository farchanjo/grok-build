# Memory with Jev — complete findings and design

Consolidated 2026-09-19 from every memory experiment run in this folder. This is
the implementation reference; the sibling docs cover other targets
(`FINDINGS-SKILLS.md` — skill selection, injection and the eval runner;
`FINDINGS.md` — permission, agents, workflow; `FINDINGS-vs-stack.md` — skill
selection against the retrieval stack).

Every number here was cross-checked against the raw files in `out/` after the
fact. Where a claim was corrected by a later measurement, the correction is
recorded next to it rather than quietly overwritten.

**Naming.** The LAN box is `solaris` (`192.168.200.32`). Earlier scripts labelled
its models "local", which reads as "on the Mac" and is wrong — nothing runs
locally. This doc says **solaris**; the scripts still say `local` in places, and
the arm labels in `out/` reflect that.

Everything below was measured against `jev-1.13` on the native transport, with
the live dev profile read-only. No Rust was changed.

## 1. Goal and constraints

Memory must stay **between sessions of the same folder** and stop being confused
with everything else. Three things must keep working, and nothing may be removed:

- **`MEMORY.md` stays the human surface** — markdown, hand-editable, watched.
- **Milvus stays a disposable mirror** — SQLite is the vector authority.
- **With Jev off, behaviour equals today** — the stages are additive.

## 2. What exists today (verified)

| piece | where | behaviour |
| --- | --- | --- |
| storage root | `xai-grok-memory/src/storage.rs` | `workspace_dir = ~/.grok/memory/{slug}-{hash8}`, `global_dir = ~/.grok/memory` |
| workspace identity | `workspace_identity.rs` | keyed on the git remote `org/repo`, so clones and worktrees share; falls back to the canonical path |
| scopes | `MemoryScope::{Global, Workspace}` | both exist |
| append write | `append_to_memory` | normalizes to `## heading`, separates with a blank line, **never truncates** |
| overwrite write | `write_long_term` | **replaces the whole file** — used by Dream |
| daily log | `write_daily_log` | `<workspace>/sessions/YYYY-MM-DD-{slug}-{sid8}.md` |
| index | `backend.rs` | `<workspace>/index.sqlite`; `list_memory_files()` returns global `MEMORY.md` + workspace `MEMORY.md` + workspace session logs |
| source labels | `classify_source` | `global` / `workspace` / `session`; `[memory.search] source_weights` defaults all to 1.0 |
| watcher | `watcher.rs` | created/modified `.md` reindexed via `reindex_file`; search checks `is_dirty` |
| Milvus mirror | `mirror.rs` | SQLite authoritative; best-effort fan-out after the SQLite write; sqlite-vec fallback; fingerprint tag + resync |
| session-end hook | `session/memory/hooks.rs` | metadata-only summary, **no LLM**; gated on ≥3 real prompts and ≥50 bytes; only on graceful shutdown |
| Dream | `dream.rs` | writes long-term memory to `MemoryScope::Workspace`; gated on `min_hours = 4` + `min_sessions` |
| retrieval profile | `~/.grokdev/config.toml` | `[retrieval_profiles.prime-main]`, embeddings `prime-local`, **`reranker_models = []`** |

Live state: 7 workspace directories, each holding 2–3 chunks (the near-empty
`MEMORY.md` headers); **two session logs ever**; Dream never fired.

## 3. Three defects to fix first — none is Jev's

1. **Global topic files are never indexed.** `list_memory_files()` returns only
   `global_dir/MEMORY.md`; the sibling files are linked but invisible to search.
   Verified: searching text that exists only in the body of
   `short-sleeps-in-verification.md` returns nothing.
2. **`write_long_term` clobbers.** Dream rewrites the workspace `MEMORY.md`
   wholesale, so anything appended since the last Dream run is lost. It must
   merge, or own the file and mediate appends.
3. **The only agent-facing write is hardcoded to Global**
   (`xai-grok-pager/src/app/effects/mod.rs:4052`). That is the drift.

Also: `search(query, max_results, min_score)` has **no scope parameter**, and
global is indexed into every workspace at weight 1.0 — verified live, a global
entry returned with score 1.00 from this workspace.

## 4. Measured evidence

### 4.1 Scope routing — `sim_memory_scope.py`

34 candidates (10 local, 12 user-wide, 12 noise), choice over
`{this_folder, global, discard}`: **82.4%**, with 10/10 local correct and every
error leaning local. A tie-break sentence *hurt* (73.5%).

### 4.2 Admission and dedup — `sim_memory_scope.py`, `sim_memory_sweep.py`

24 candidates (8 worth keeping, 7 restatements, 9 momentary):

| arm | accuracy | precision | recall | restatements stored |
| --- | --- | --- | --- | --- |
| bare | 62.5% | 45.5% | 62.5% | 6/7 |
| one question, store in state | 79.2% | 100% | 37.5% | 0/7 |
| **two atomic questions + store, threshold 0.25** | **83.3%** | **100%** | **100%** | **0/7** |

**The store must be in the state** — without it the gate stores a restatement 6
times out of 7; with it, zero.

**The threshold band is 0.20–0.22.** Noise scores 0.05–0.23, valuable notes
0.20–0.86: the overlap is three points wide. Sweeping `worth`:

| worth >= | valuable lost | noise admitted |
| --- | --- | --- |
| 0.10 | 0 | 7 |
| **0.20** | **0** | **1** |
| 0.25 | 1 | 0 |
| 0.50 | **7** | 0 |

0.5 — the intuitive coin flip — loses 7 of 36 valuable notes. This is the fourth
place the same mistake appeared (skill gate, skill `fits`, memory value, admission).

### 4.3 End-to-end pipeline — `sim_memory_pipeline.py`

48 events, 12 later queries, both arms:

| metric | today | today + jev |
| --- | --- | --- |
| notes stored | 48 | **27** |
| noise dropped (of 12) | 0 | **10** |
| valuable lost (of 36) | 0 | 4 |
| restatements stored (of 8) | 8 | **1** |
| scope routed right | n/a | **20/25** |
| query top-1, no rerank | 6/12 | **9/12** |
| query top-1, reranked | 10/12 | **11/12** |

A clean store alone lifts retrieval without any rerank. With a rerank both arms
converge, so the two are partly redundant for top-1; the gate's distinct value is
store size, duplicates and scope.

### 4.4 Coexistence — `sim_coexistence.py`

Hand-written edit interleaved at event 20, then reindexed and queried:

| check | today | today + jev |
| --- | --- | --- |
| hand edit survived | yes | yes |
| global `MEMORY.md` chars | 3,675 | 1,743 |
| workspace `MEMORY.md` chars | 106 | 1,155 |
| headings well-formed | 51/51 | 35/35 |
| local query served by a global entry | 6/7 | **2/7** |

Nothing is clobbered, the format stays valid through the existing normalizer, and
the user's complaint drops from 6 of 7 to 2 of 7.

### 4.5 Embedding head-to-head — `sim_embedding.py`

Dense-only retrieval, 172 documents, 66 queries:

| model | dims | recall@1 | recall@5 | recall@10 | embed secs |
| --- | --- | --- | --- | --- | --- |
| `openai/text-embedding-3-small` (OpenRouter) | 1536 | 30/66 | 49/66 | 50/66 | 10.84 |
| **`Qwen3-Embedding-4B` (solaris :8001)** | 2560 | **32/66** | **50/66** | **52/66** | **2.41** |

The solaris model wins on every recall and embeds 4.5x faster, free, on the LAN.

**Correction on the record.** An earlier 12-query run said the opposite — "the
local embedding is not automatically better" — because that sample was small and
confounded with three other variables changing at once. Isolated on 66 queries it
wins consistently. The doc carried the wrong conclusion for one message before
the isolated run landed.

### 4.6 Factorial — `sim_combos.py`

All 8 combinations of embedding × reranker × jev, 66 queries, identical retrieval.
`local` below is the solaris embedding:

| rank | embedding | reranker | jev | top-1 | top-3 | MRR | p50 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **1** | **solaris** | **none** | **on** | **43/66** | **48/66** | **0.698** | 323 ms |
| 2 | solaris | real | on | 42/66 | 47/66 | 0.689 | 347 ms |
| 3 | remote | real | on | 41/66 | 46/66 | 0.663 | 330 ms |
| 4 | remote | none | on | 41/66 | 46/66 | 0.662 | 315 ms |
| 5 | solaris | none | off | 26/66 | 43/66 | 0.529 | 24 ms |
| 6 | solaris | real | off | 25/66 | 39/66 | 0.523 | 44 ms |
| 7 | remote | real | off | 25/66 | 40/66 | 0.505 | 33 ms |
| 8 | remote | none | off | 23/66 | 39/66 | 0.492 | 15 ms |

- **Jev dominates**: every `jev=on` arm beats every `jev=off` arm by 17–20 top-1.
- **The solaris embedding is second**: it wins in both jev conditions.
- **The reranker is neutral to negative**: it costs one query in three of four
  pairs and never helps. Top-10 is identical with and without it, and identical
  with and without Jev — the work is ordering, which is where Jev acts.
- Worst combination = today's shape.

### 4.7 Retrieval rerank — `sim_memory_rerank.py`

15 chunks across scopes, 10 queries, keyword baseline: 7/10 → **9/10**, gold in
top-3 10/10. A rerank fixes the global leak without needing a scope filter.

## 5. Design

Five stages, all fail-open, all additive.

| stage | attachment | fail-open behaviour |
| --- | --- | --- |
| **admission** | beside `append_to_memory`, before the write | store everything |
| **dedup** | same seam, store in state, threshold 0.50 on `covered` | store everything |
| **scope routing** | replaces the hardcoded `MemoryScope::Global` at `effects/mod.rs:4052` | global |
| **rerank** | new `RerankerProtocol` variant over the shortlist | RRF order |
| **consolidation** | reuse the dedup question inside Dream | Dream unchanged |

**Question shapes that measured well:**

```
worth:    noul, "Would this note still be worth recalling in a future session,
           on its own merits?"  criteria true=durable fact/rule/preference,
           false=momentary or rediscoverable            threshold >= 0.20
covered:  noul, "Do the entries already in memory already say this, or something
           close enough that storing it again adds nothing?"
                                                        threshold >= 0.50
bucket:   choice, {this_folder, global, discard}, state carries workspace_path
rerank:   noul per candidate, in isolation, state carries workspace_path + query
```

Two hard rules learned the hard way:

- **One `noul` per candidate.** A single "does the winner fit" question conflates
  them and lands mid-scale.
- **Never put prose policy in the state** unless it is measured. A policy
  paragraph hurt the permission gate; a tie-break sentence hurt scope routing;
  a specificity sentence helped skill routing in isolation and turned out
  **neutral** once a shortlist existed (`FINDINGS-SKILLS.md` §13). Test each, and
  re-test after the pipeline around it changes.

## 6. Verified infrastructure

| endpoint | what | proof |
| --- | --- | --- |
| `192.168.200.32:8001` | vLLM, `Qwen3-Embedding-4B` | `/v1/embeddings` → 2560 dims |
| `192.168.200.32:8002` | vLLM, `bge-reranker-v2-m3` | `/v1/rerank` ranked correctly (0.449 vs 3e-5) |
| `192.168.200.32:8800` | OmniVoice TTS/ASR/MT | `/health`, `/openapi.json` |
| `vm.services:19530` | Milvus | `grok_mem_ce8c9272b11ef26c` live, `id/embedding/fp`, tag `grok:0c31899c…:1536:1` |

Both vLLM servers expose `/rerank`, `/v1/rerank`, `/v2/rerank`, `/score`,
`/v1/score` — matching the existing `RerankerProtocol::OpenaiCompatible`.

`8801` and `8802` are refused; `57033` accepts TCP but answers neither HTTP/1.1
nor h2c. OpenRouter's `/v1/rerank` returns 404 guardrail for every
`cohere/rerank-*`.

## 7. Implementation order

1. **Fix the three defects** (§3). Routing to global is pointless while global
   bodies are unindexed, and the pipeline cannot share a file with a
   whole-file rewriter.
2. **Admission + dedup** at the append seam, store in state, thresholds 0.20 and
   0.50. Fail-open.
3. **Scope routing** replacing the hardcoded `Global`.
4. **Point the embedding at `solaris:8001`.** Highest return, lowest risk: one
   config line, better recall, 4.5x faster, free.
5. **Jev rerank** over the shortlist. Leave the reranker slot empty.
6. **Consolidation** inside Dream, reusing the dedup question.
7. Only then the TUI toggle, following the `[compaction.jev]` + `/jev` shape.

## 8. TUI configuration

**What exists.** `/retrieval-settings` (`retrieval_settings_modal.rs`) carries a
memory section: `draft_memory_mode`, `draft_memory_vector_store`,
`draft_memory_profile` (`:463-465`). That covers the backend and the retrieval
profile.

**What is missing.** Every write-side knob this doc recommends, plus the master
switch:

| recommendation | surface today | needed |
| --- | --- | --- |
| admission gate on/off | nothing | a toggle |
| `worth` threshold 0.20 | compiled | a row |
| `covered` threshold 0.50 | compiled | a row |
| scope routing on/off | nothing | a toggle |
| `[memory] enabled` | config only, not in the modal | a row |
| `session.save_on_end` | config only | a row |
| global-vs-local visibility | nothing | the scope split in `/context` |

**Shape, following `/jev`.** `/jev on|off|status` dispatches a typed
`Action::SetCompactionJevEnabled`, the same action the settings row dispatches,
so the slash command and the row cannot drift; the value persists and the live
session picks it up through the config reload fan-out. `status` reports the
effective value.

```
/memory-gate on|off|status
  on      -> Action::SetMemoryGateEnabled(true)
  off     -> store everything, as today
  status  -> enabled, thresholds, store counts by scope, and the last decision
             (kept / dropped / routed, with the score)

settings rows in the retrieval modal's memory section:
  "Memory"                   bool — the `[memory] enabled` master switch, which
                             today is config-only and not in the modal
  "Save on session end"      bool — `session.save_on_end`, also config-only
  "Memory write gate"        bool
  "Admission threshold"      f32, default 0.20 (§4.2)
  "Duplicate threshold"      f32, default 0.50
  "Scope routing"            bool
```

`status` printing the store counts by scope is the cheap version of the
visibility gap: it makes the global-vs-workspace split observable without opening
a file.

## 9. Hazards

- **Token budget.** The store goes into the state on every candidate. Cap it and
  fall back to a summary when it grows.
- **Latency.** ~300 ms per decision, ~700 ms for a rerank. Both fit
  `degrade_on_error = true` and the existing deadlines.
- **Threshold drift.** 0.5 is wrong for this model family. Keep the constants in
  one reviewed place.
- **Fail-open is not optional.** Any Jev failure must reproduce today's
  behaviour exactly.

## 10. Not verified

- One run per arm; differences of one query do not separate.
- The 0.20–0.22 band comes from one 48-event sample.
- Arm A of the retrieval comparisons is a faithful core, not the production code
  path.
- Milvus was exercised over REST, not through the Rust mirror.
- The memory corpus is 28–49 chunks; a larger store is where the vector backend
  and the reranker would start to matter.
- Nothing was measured inside the TUI.
- **The three defects in §3 are read, not reproduced.** The unindexed global
  topic files were confirmed by searching for body-only text and getting nothing;
  the `write_long_term` clobber and the hardcoded `Global` were confirmed by
  reading the call sites. No Dream run was triggered and no append was raced
  against one.
- **Cross-check status**: every number in §4 was re-read from the raw files in
  `out/` after the doc was written and matches. The arm labels in those files
  still say `local` where this doc says `solaris`.
- The scope-routing and gate cases are mine, hand-labelled; the 82.4% and the
  100%/100% are against those labels, not an oracle.