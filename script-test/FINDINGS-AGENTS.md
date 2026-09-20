# Agent recommendation and selection with Jev — complete findings and design

Consolidated 2026-09-19. Companion to `FINDINGS-PERMISSION.md`,
`FINDINGS-LAZINESS.md`, `FINDINGS-SKILLS.md` and `FINDINGS-MEMORY.md`;
`FINDINGS.md` is the umbrella.

Measured against `jev-1.13`, native transport, live dev profile read-only. No
Rust was changed.

## 1. Goal

Decide where the agent choice belongs: **select** it (the harness or the parent
picks) or **inject** a recommendation (advisory metadata the parent reads), and
whether `reasoning_effort` can be chosen at all.

## 2. What exists today (verified)

`session/prime/agents.rs` (PR20) is the recommender, and it is carefully built:

- ranks the authoritative callable-agent snapshot against the current prompt and
  the already-selected skill names;
- optionally refines the indexed inventory through FTS + sqlite-vec KNN, metadata
  only — canonical name, bounded public description, safe source class; **never an
  agent prompt body**;
- **intersects and revalidates every survivor against a fresh live
  `CallableAgentAuthority`** built from the same session/spawn data the Task tool
  uses, so index presence is never authority and omitted eligibility can never
  mean allowed;
- renders an **advisory-only** block — it never spawns, never enqueues, never
  touches allowlists or depth;
- pinned recommendations are never displaced; the current agent is excluded;
  budgets come from `AgentPrimeConfig`.

So the recommendation exists and is advisory. The **selection** is the parent's,
at `task` time, from the tool schema.

`reasoning_effort` is a free string on the task input; the real enum is
`None, Minimal, Low, Medium (default), High, Xhigh, Max`. **No agent frontmatter
declares one**, so effort is entirely the parent's choice.

## 3. Measured: select versus inject

25 delegation cases, the roster of 109 profile agents plus the three built-ins:

| arm | type correct | gold in the injected shortlist | injected tokens/turn |
| --- | --- | --- | --- |
| select from the full roster | 19/25 (76.0%) | — | 0 |
| **inject top-3, then select** | 20/25 (80.0%) | **20/25 (80%)** | 150 |
| select, no advisory block | 19/25 (76.0%) | — | 0 |

- **The advisory block buys one case.** +1 at 150 tokens per turn is inside the
  noise of a 25-case set.
- **Its recall is the ceiling.** The recommender puts the gold in the injected
  three 20 times out of 25, so a selection that reads only the shortlist can never
  beat 80% — and the arm reaches exactly that.
- **Selecting from the full roster is free and equal.** 76% with no injected text.

The honest reading: the block does not pay for itself *in selection accuracy on
this fixture set*. Its value has to come from somewhere else — narrowing for a
weaker parent, or carrying the revalidation result that the raw schema does not —
and that was not measured here.

The six misses on the full roster are all defensible synonyms, so 76% is a floor:
`code-reviewer` → `rust-engineer`, `debugger` → `error-detective`, `python-pro` →
`data-engineer`, `postgres-pro` → `database-optimizer`, `test-automator` →
`qa-expert`, `incident-responder` → `error-detective`.

## 4. Measured: effort is worse than a constant

With the real seven-level enum collapsed to four and a rubric that says to pick
the lowest sufficient level:

**14/25 (56%).** The distribution is 21 `high`, 3 `medium`, 1 `max`.

16 of the 25 golds are `high`. **A constant "high" answer scores 16/25 (64%) —
better than the question.** The rubric is not merely weak, it is actively worse
than not asking.

That is the second measurement of this: the earlier four-option rubric returned
`high` on 23 of 25. The question has now been asked two ways and collapsed both
times.

## 5. The corpus carries an unused routing graph

Read-only analysis of the 109 shipped agents.

**What the corpus is.** Uniform frontmatter on all 109: `name`, `description`,
`capabilityMode`, `skills`. Three findings from that alone:

- **`skills: []` on every agent** — the field exists and is empty everywhere.
- **No `model` and no `reasoning_effort` on any agent** — which is why the effort
  question lands on the parent (§4).
- **`capabilityMode` is 107 `read-write` and 2 `all`** — nearly constant.
- Descriptions run 144–353 chars, median 246, so unlike skills they are never
  truncated by a listing bound.

**What the corpus declares and nothing reads.** 108 of 109 agents carry a
`## Delegates` section — a hand-written routing map:

```
code-reviewer:   architect-review -> architect-reviewer; qa -> qa-expert;
                 refactor -> refactoring-specialist
virtualization-stack: libvirt -> virt-libvirt; ovs -> virt-ovs-doca;
                 ovn -> virt-ovn; integration -> virt-integration
```

332 edges, 113 distinct targets. `grep Delegates` across the shell, tools and
agent crates finds only unrelated prose uses — **no code reads it**.

The graph is real but not clean: **14 targets do not resolve**, and they are
language names where the agent is `X-architect` (`-> java`, `-> golang`,
`-> csharp`, `-> kotlin`, …). Ten agents have no inbound edge, so five roots reach
91 of 109. There are cycles (`java-architect` → `kotlin-specialist` →
`spring-boot-engineer` → `java-architect`).

**As a router it beats the flat choice.**

| arm | type correct | entry had delegates | narrowing |
| --- | --- | --- | --- |
| flat, pick from 109 | 20/25 (80.0%) | — | 109 candidates |
| **graph: pick an entry, then pick among it and its delegates** | **22/25 (88.0%)** | 22/25 | **2.7 candidates** |

**+8 points, and the second question is not what earns them** — the entry agent is
already right in most cases; the graph's value is that the entry's declared
delegates put the right specialist in front of the chooser. On the 22 cases where
the graph acted it scores 91%.

### 5.1 Correction: the graph substitutes for retrieval

The +8 was measured against a flat choice over all 109 agents — which is what you
have with **no retriever**. Run the full space and the graph goes flat:

| axis | mean top-1 across arms |
| --- | --- |
| retrieve: milvus | **19.4** |
| retrieve: hybrid | 17.2 |
| retrieve: native (lexical) | 11.5 |
| jev: on | 17.8 |
| jev: off | 14.3 |
| rerank: none | 16.2 |
| rerank: real | 15.9 |
| **graph: on** | **16.0** |
| **graph: off** | **16.1** |

24 arms, 25 cases, only solaris models. Best arm `milvus/none/jev=on/graph=on` at
21/25; the same arm without the graph is 20/25.

So the honest ordering is: **retrieval is the dominant axis (+8 for dense over
lexical), Jev is second (+3.5), the reranker is neutral-to-negative, and the graph
is worth about one case once a retriever exists.** It substitutes for retrieval
quality the same way the cross-encoder does in the skill space.

What the graph still gives that the numbers do not: a 2.7-candidate shortlist with
the entry's own declared structure, which is cheaper to render than ten
descriptions and auditable by a human reading the agent's `## Delegates` line.

### 5.2 The best combinations, and what each one costs

Ranking the 24 arms by accuracy hides that **eight of them sit within one case of
the best** — the top is a plateau, not a winner. The Pareto frontier is the
useful view:

| combination | type correct | tokens/turn | p50 | what it buys |
| --- | --- | --- | --- | --- |
| native / no jev / no rerank / no graph | 11/25 | 0 | 0 ms | the free baseline |
| **milvus / no jev / no rerank / no graph** | **18/25** | **0** | 9 ms | **+7 for free** |
| milvus / no jev / jev=on / no graph | 20/25 | 1,185 | 632 ms | +2 for the Jev cost |
| milvus / reranker / jev=on / no graph | 21/25 | 1,185 | 592 ms | +1 more, same tokens |

Read it as a value ladder:

1. **The embedding is the whole win, and it is free.** +7 cases, zero tokens,
   9 ms, no model call. `milvus/none/jev=off` at 18/25 beats every arm that has
   Jev but no retriever, and beats the lexical baseline by seven.
2. **Jev adds +2** on top of a perfect pool, for 1,185 tokens and ~600 ms per
   turn. Real but priced.
3. **The reranker adds +1** at no extra tokens. It is the cheapest of the three
   add-ons and the least reliable — the same axis measured *negative* in the skill
   space and negative on the mean here.
4. **The graph is the worst deal in the table**: +485 tokens per turn for ±0–1
   case. It is a rendering choice (a 2.7-item auditable shortlist) more than an
   accuracy one.

So the recommended combination depends on what is being bought:

- **If the question is value:** `milvus` alone. 18/25, free, no model call, and it
  already puts the gold in the pool 25 of 25 times.
- **If the question is maximum accuracy:** `milvus + reranker + jev`, no graph.
  21/25, and accept 1,185 tokens and ~600 ms per turn.
- **The graph is optional either way.**

Retrieval is near-perfect on every dense arm — the gold is in the top-10 pool
**25 of 25 times** for milvus and hybrid, against 14 of 25 for lexical. Everything
downstream is ordering, and ordering is worth at most 3 cases on top of a perfect
pool. That is the ceiling of this problem, and it is small.

## 6. Where the corpus itself can be improved

Four checks, all read-only. The first two are real; the third is mostly a false
positive of the check, and the reason it is a false positive is the interesting
part.

**1. Seven agents read read-only but can write.** `capabilityMode` is `read-write`
on 107 and `all` on 2 — nothing is read-only. Against a regex on the description,
seven read as read-only: `code-reviewer`, `architect-reviewer`,
`compliance-auditor`, `chaos-engineer`, `data-scientist`, `virt-integration`,
`qa-expert`. A reviewer that can edit the code it reviews is a design question,
not a bug, but it is worth an explicit answer rather than a default.

**2. The delegation graph is 15 edges short.** 14 targets do not resolve, and
**11 of them have an obvious agent**: `-> java` should be `java-architect`,
`-> golang` → `golang-pro`, `-> swift` → `swift-expert`, and so on. The remaining
three point at agents that **do not exist at all** — `ruby`, `ansible`, `dart` —
which is a signal about the corpus's coverage rather than a typo.

**3. The descriptions name their specialty, but not in the slug.** Eleven agents
look indiscriminate to a literal check; on inspection ten are false positives:
`cpp-pro` says "C++", `golang-pro` says "Go", `csharp-developer` says "C#",
`dx-optimizer` says "developer experience". Only `fullstack-developer` is
genuinely vague — "End-to-end feature owner with expertise across the entire
stack" never says *fullstack*.

This is the **opposite of the skill corpus**, which is written for a substring
matcher (inline `TRIGGER:` lists, the keyword repeated verbatim). Agent
descriptions are written for a human reader. A semantic chooser is fine with
that; a keyword matcher would score them badly.

**4. The distinguishing token is rarely the first one.** Name tokens cluster hard:
`engineer` groups 21 agents, `virt` 12, `developer`/`expert`/`pro`/`architect` 9
each. So `python-pro` and `golang-pro` are distinguished by the *first* token and
nothing else, while `architect-reviewer` and `cloud-architect` share both
directions. A chooser keying on the leading token is guessing.

**5. `skills: []` on all 109.** The field exists and is empty everywhere. Prime's
recommender already ranks agents *against the selected skill names*, so the
binding exists in one direction; the agent's own declaration is unused.

## 7. Design

| stage | attachment | fail-open |
| --- | --- | --- |
| recommendation | unchanged — advisory, revalidated, never spawns | today's behaviour |
| selection | unchanged — the parent picks at `task` time | today's behaviour |
| effort | **drop the question**, default `medium` and let the agent's own loop escalate | constant `medium` |

Two candidates for the effort fix, neither measured:

1. **Drop it.** A constant beats the question, so the question is not earning its
   tokens. `ReasoningEffort::Medium` is already the default.
2. **Make it a property of the agent, not of the task.** No agent frontmatter
   declares an effort today; a `reasoning_effort:` field per agent would make it
   deterministic and inspectable, and the parent would stop guessing. This is the
   more promising direction because it removes a decision rather than tuning one.

For the recommendation, the measurement says: **do not make it load-bearing**.
Keep it advisory. If it is to earn its place, the injected block should carry what
the schema does not — that the candidate was revalidated against live authority
this turn, and that the rest of the roster was not.

## 8. TUI configuration

**What exists.** `AgentPrimeConfig` is edited from `/retrieval-settings`
(`retrieval_settings_modal.rs` builds it at `:1421`) alongside the skill and
memory sections — `enabled`, `retrieval_profile`, `max_results`,
`max_body_chars`, `max_total_chars`, `max_tokens`, `max_context_fraction`,
`deadline_ms`, `degrade_on_error`. `subagents_enabled` and the per-agent toggles
live in the settings registry.

**What is missing.**

| recommendation | surface today | needed |
| --- | --- | --- |
| recommendation on/off | via the retrieval modal | a fast toggle |
| recommendation visibility | nothing in the transcript | which agents were recommended, and their count |
| effort policy | nothing | a per-agent `reasoning_effort` field, or a global default row |

**Shape, following `/jev`.** `/jev on|off|status` dispatches a typed
`Action::SetCompactionJevEnabled`, the same action the settings row dispatches,
so the slash command and the row cannot drift; the value persists and the live
session picks it up through the config reload fan-out.

```
/agents-recommend on|off|status
  status  -> enabled, recommended names this turn, injected tokens

settings rows: "Recommend agents" bool beside the prime rows;
               "Follow the delegates graph" bool (§5.2 — off by default, since
               it measured ±0–1 case on top of retrieval and costs 485 tok/turn);
               "Default reasoning effort" enum, if §5 option 2 lands
```

## 9. Implementation order

1. **Point the retrieval at the solaris embedding.** +7 cases for free, zero
   tokens, 9 ms, and the gold in the pool 25 of 25 times. §5.2.
2. **Decide whether Jev is worth 1,185 tokens and ~600 ms per turn for +2 cases**
   on top of it. If it is, add the reranker too — it is +1 at no token cost.
3. **Treat the `## Delegates` graph as optional** (§5.2) — +485 tokens per turn for
   ±0–1 case. Wire it for the auditable 2.7-item shortlist, not for the score.
4. **Fix the 11 fixable dangling targets** and decide the 3 that point at agents
   which do not exist (`ruby`, `ansible`, `dart`) — either write those agents or
   re-point the edges.
5. **Stop asking for effort.** A constant `high` scores 64% against 56% for the
   question. One line, immediate.
6. **Try effort as an agent property.** Add `reasoning_effort:` to agent
   frontmatter and measure; it removes a decision instead of tuning one.
7. **Decide whether the advisory block earns 150 tokens/turn.** It buys one case
   today. Either give it something the schema lacks (the revalidation result) or
   make it cheaper.
8. **Answer the seven read-only-but-writable agents** (§6.1). A reviewer with
   write access is a choice, not an accident, but it should be a stated one.
9. **Surface what was recommended.** `/agents-recommend status`, and a line when
   the block is injected.

## 10. Hazards

- **The recommender's recall is the selection's ceiling.** At 80%, no amount of
  selection tuning gets past it. Measure recall, not just final accuracy.
- **A recommendation that never spawns is easy to mistake for a decision.** The
  block says so in prose; the UI does not.
- **Effort interacts with cost.** A global `high` default raises latency
  everywhere; a per-agent field is safer than a global row.
- **The 25 cases are mine**, with several defensible synonyms, so both the 76% and
  the 80% are floors.

## 11. Not verified

- **The parent was not measured.** Both selection arms are Jev standing in for the
  delegating model; production has a strong model reading a tool schema, which may
  respond differently to the advisory block than a choice question does.
- 25 cases; the +1 between 76% and 80% does not separate.
- The effort golds are my judgement, and they are 16/25 `high` — which is itself
  evidence that the levels are coarse for this workload.
- The recommender's own ranking was approximated: one `choice` call, not the real
  FTS + KNN + RRF + revalidation path.
- `validate_subagent_type` and the authority capture were read, not exercised.
- Nothing was measured inside the TUI.