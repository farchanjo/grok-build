# Workflows with Jev — complete findings and design

Consolidated 2026-09-19. Companion to the other `FINDINGS-*.md`; `FINDINGS.md` is
the umbrella.

Measured against `jev-1.13`, native transport, live dev profile read-only. No
Rust was changed.

## 1. Goal

Find where Jev can improve workflow authoring and launching — knowing that a
workflow is a Rhai script, that the model writes it, and that the engine cannot
call a model.

## 2. What exists today (verified)

| piece | where | behaviour |
| --- | --- | --- |
| engine | `xai-workflow/src/engine.rs` (1880 lines) | Rhai host: `agent`, `parallel`, `budget`, `complete`, `exit`, `fingerprint`, `json_encode`, `log`, `phase`, `sleep`, `timestamp` |
| meta | `xai-workflow/src/meta.rs` | name lowercase/hyphen/≤64 bytes, description ≤1024, `when_to_use` ≤2048, phases ≤64 with unique titles ≤128; `meta` must be the first statement, a pure literal |
| registry | `session/workflow/registry.rs` | `BUILTIN_WORKFLOWS` with **exactly one** entry (`deep-research`, `include_str!`), plus file discovery from `.grok/workflows/` and `~/.grok/workflows/` |
| launch | `grok_build/workflow/mod.rs` | top-level sessions only; `agent_budget` default 128; `validate_only` smoke check |
| TUI | `/workflows` | lists **runs**, and `/workflow pause\|resume\|stop <name>` |

Three findings from that alone:

1. **No host function calls a model.** A workflow reaches an LLM only through
   `agent()`, which spawns a subagent — slow and expensive. Nothing equivalent to
   a one-call decision exists.
2. **The `workflow` tool never says which workflows exist.** Its description says
   *"Prefer a registered workflow when one fits"* and *"a registered workflow
   (built-in, or discovered from the project `.grok/workflows/` or user
   `~/.grok/workflows/`)"* — but no catalog reaches the prompt.
3. **The `create-workflow` skill is referenced and not installed.** The tool
   description says *"Before writing or editing a script, read the
   `create-workflow` skill's SKILL.md"*, and `~/.grok/skills/create-workflow/`
   does not exist.

Live state: `~/.grok/workflows/` is **empty**; the project has one file
(`.grok/workflows/provider-factory.rhai`, 28 KB).

## 3. Measured: the discovery gap

Three arms on 8 requests, the catalog being the one built-in plus four plausible
user workflows:

| arm | correct |
| --- | --- |
| blind — no catalog at all (today) | 5/8 |
| names — the names, no descriptions | **7/8** |
| listed — names with descriptions | **8/8** |

**A bare list of names recovers almost all of it.** The descriptions add one more
case, and the whole fix is a static catalog in the tool description — the same
shape skills already have, at a fraction of the size (five entries against 172).

Caveat that matters: the "blind" arm was constructed by handing the model the
candidate name and asking whether it fits, so it measures *recognition*, not
generation. Today's real shape is worse than 5/8 — the model has to recall the
name from nothing.

## 4. Measured: retrieval beats a listing once the catalog grows

§3 measured discovery with a five-entry catalog. A listing is cheap at five and
expensive at fifty, and it rides the tool description **every turn whether or not
a workflow is launched**. So this runs the same requests against a 30-entry
catalog, including the retrieval stack.

| arm | top-1 | catalog tok/turn | selection tok/call | p50 |
| --- | --- | --- | --- | --- |
| blind (no catalog) | 23/24 | **0** | 299 | 306 ms |
| listed-5 (partial) | **5/24** | 125 | 462 | 293 ms |
| names-30 (no descriptions) | 23/24 | 95 | 557 | 301 ms |
| listed-30 | 24/24 | 677 | 1,144 | 292 ms |
| **milvus + jev** | **24/24** | **0** | 453 | 326 ms |
| milvus + reranker + jev | 24/24 | 0 | 453 | 333 ms |

- **Retrieval strictly dominates the listing**: same 24/24, **677 fewer tokens
  every turn**, and 691 fewer per selection. Nothing is lost and the per-turn cost
  goes to zero.
- **A partial listing is the worst arm** (5/24) — worse than no catalog at all,
  because the five on offer are usually not the right five.
- **The reranker changes nothing** — identical accuracy and tokens, +7 ms. Fifth
  time the axis has been neutral-to-negative.
- **The blind arm is flattered by its construction.** It hands the model the
  candidate name and asks whether it fits, which measures recognition, not
  generation — the same caveat as §3.

So the recommendation flips with catalog size: with today's single built-in
either works; **once there is a catalog worth the name, embed and let Jev pick
from the shortlist, and do not ship the listing.**

## 5. Measured: naming is weak, and the earlier number was inflated

15 fixtures with four candidate names each, **candidates shuffled** so the gold is
not always first:

| measure | result |
| --- | --- |
| gold picked | 6/15 (40%) |
| picks that satisfy the meta rules | **15/15** |

Two readings:

- **40% against a 25% chance baseline** is weak. The earlier 53.3% was confounded:
  the gold sat at position 0 in 14 of the 15 fixtures, so a position-following
  answer would have scored well.
- **Jev respects the naming rules perfectly** once they are in the question —
  15 of 15 valid, no `--`, no uppercase, no overrun. The rules are not the
  problem; choosing among four plausible names is.

## 6. Measured: a workflow names an agent per step, blind

`AgentOpts` carries `agent_type: Option<String>` and that is the whole interface.
Three things about it:

1. **It is passed straight through**, `unwrap_or("general-purpose")`
   (`host_service.rs:459`), and validated only at spawn —
   `validate_subagent_type` runs in `subagent_coordinator.rs:315`, so a typo
   surfaces **mid-run**, not at authoring.
2. **`validate_only` does not catch it either**: its smoke check runs
   *"metadata, compile, one canned-host path"*, and a canned host never resolves
   an agent type. A misspelled `agent_type` passes the check and fails in
   production.
3. **The built-in never uses it.** `deep_research.rhai` writes `agent_type` zero
   times and re-describes every role in prose inside the prompt.

And there is **no `skills_hint`** on `AgentOpts`, though `spawn_subagent` has one —
so a workflow cannot bind a skill to a step at all.

| arm | type correct | cost per step |
| --- | --- | --- |
| blind — no catalog, recognition construction | 23/25 | 0 |
| listed — 109 entries in the prompt | 20/25 | 109 entries |
| retrieval + Jev — KNN top 5, then pick | 20/25 | 1 KNN + 1 Jev call |

**Giving the author the catalog does not improve the choice** (20/25 either way),
and the blind arm's 23/25 is again the flattered recognition construction — the
model is handed the name and asked whether it fits, which is not what an author
does. So the authoring accuracy is ~80%, and the agent graph (which
`FINDINGS-AGENTS.md` §5 measured at 88%) is the untested lever here.

**The improvement is therefore not "help the author choose" but "catch the typo
and widen the interface":**

| gap | fix |
| --- | --- |
| a typo fails mid-run | validate every `agent_type` literal in `validate_only`, with the canned host extended to resolve types |
| roles re-described in prose | use `agent_type` in the built-in, and say so in the tool description |
| no skill binding | add `skills_hint: Option<Vec<String>>` to `AgentOpts`, mirroring `spawn_subagent` |

## 7. A generated workflow, using the stack on itself

`gen_monitor_workflow.py` emits a working script and validates it. It is a
demonstration that also happens to be useful: a monitor for a job another session
started, which is the actual shape of work in this repository.

**How the generator makes its choices** — each step uses the layer the findings
recommend for it:

| step | layer | evidence |
| --- | --- | --- |
| which phases | embedding ranks the pool, reranker reorders, top-k kept in canonical order | `FINDINGS-SKILLS.md` §8 |
| the workflow name | Jev picks from five candidates under the real meta rules | §5 — naming is weak, so candidates rather than generation |
| the emitted script | validated against `meta.rs`'s rules and the lock contract | §2 |

**What the workflow does.** Phases Observe → Judge → Diagnose → Report. It polls
with `sleep`, has an agent classify each poll into
`progressing | stopped | blocked | finished` with an evidence string, breaks on two
consecutive non-progressing observations, diagnoses only when stopped or blocked,
and `complete`s with `needs_human: true` when the diagnosis is missing or
low-confidence.

**How it avoids the lock.** Every agent is `capability_mode: "read-only"` and its
prompt carries the contract verbatim:

```
NEVER run cargo, make, or anything that builds: another session holds the
target-dev lock and a second build would block on it.
```

The workflow never builds, so it cannot contend with the session it is watching —
which is the point of watching rather than re-running.

**What it does not use.** `parallel` (a monitor is sequential), and `decide`
(§11.1) because the engine does not have it yet — the two judgement steps go
through `agent()` instead. When `decide` lands, the classifier step is the first
thing that should move to it: a 300 ms call replacing a subagent spawn.

Output: `monitor-run.rhai`, 7.5 KB, four phases, validation clean on name length
and charset, description and `when_to_use` bounds, phase count and uniqueness,
every `phase()` and `phase:` referencing a declared phase, and the read-only
contract present.

## 8. Parallelism, and where the real constraint is

The plan workflow runs four jobs in parallel in Phase 2. That was evaluated against
the two things that could break it, and only one of them can.

**The lock is not the constraint.** Implementation agents do not build — the brief
says so — so four of them can edit at once without ever touching `./target-dev`.
The lock constrains only the two phases that build (Verify and TUI validation),
and both are serial by construction.

**Shared files are the constraint.** Extracting every file each group's findings
cite and intersecting them:

| pair | shared |
| --- | --- |
| skills × memory | `retrieval_settings_modal.rs` |
| skills × agents-todo-workflow-goal | `retrieval_settings_modal.rs` |
| memory × agents-todo-workflow-goal | `retrieval_settings_modal.rs` |
| permission-laziness × agents-todo-workflow-goal | `settings/defs.rs` |

**Four of six pairs collide, on two files — and both are TUI files.** Two agents
editing one file concurrently is the only way this fan-out breaks, and a lost edit
is silent.

**The fix is ownership, not fewer agents.** The TUI surface is already Phase 4's
job, so each parallel brief now carries:

```
FILES YOU OWN: <the group's files>
DO NOT EDIT retrieval_settings_modal.rs or settings/defs.rs — the TUI phase owns
them and edits them serially.
```

Phase 2 becomes collision-free by construction, and the settings work happens once,
in one place, after the groups that would have fought over it.

**The honest ceiling:** parallelism here is bounded by the shared-file surface, not
by the machine or the lock. Four is right for this plan because the four targets are
genuinely disjoint once the TUI is carved out; a fifth group would have to find a
fifth disjoint surface, and the crates overlap too broadly for that today.

## 9. TUI validation, in tmux

The plan touches the console — new slash commands, new settings rows, an injection
marker — and a console change is not verified by tests. The workflow's last working
phase drives the built binary in a detached tmux session, following the repository's
own rules exactly:

- one session named `grok-dev`, `-x 200 -y 50`, never `tmux attach`;
- the canonical environment inline, with every Cursor/Claude/Codex discovery flag at 0;
- built first, run with `--no-leader --no-auto-update`;
- read with `capture-pane` and treated as ground truth, driven with `send-keys`;
- **killed at the end**, in the brief and again by a separate cleanup agent, because a
  session left alive holds the development profile.

It skips itself when no file under the pager crate changed, checks each new command
and row in order, toggles one switch off and back, and reports pass/fail with the
captured line as evidence. It runs after Verify because it builds, so the two are
serial.

## 10. Does embedding or Jev help the workflow itself?

Asked of the plan workflow, and the answer is mostly no — with a reason worth
recording.

### 10.1 Embedding does not reproduce the fan-out grouping

The four parallel jobs and their file ownership were hand-written. Clustering the
plan's 32 Phase-2 item titles by embedding and comparing:

| grouping | clusters | agreement with the hand split | pairs colliding on files |
| --- | --- | --- | --- |
| hand (by target) | 4 balanced | — | **0** |
| embedding (agglomerative) | 1 × 29 + 3 × 1 | 18/32 | 3, over 24 files |

The embedding collapses almost everything into one cluster. The reason is in the
similarity distribution:

| corpus | median similarity | max | pairs above 0.7 |
| --- | --- | --- | --- |
| the plan's item titles | 0.410 | 0.769 | **1 of 496** |
| agent descriptions | 0.559 | 0.897 | — |

**One-line item titles share generic vocabulary** — "gate", "threshold", "decision",
"add" — so there is no cluster structure to find. The hand split wins because it
uses a signal the embedding does not have: **which crate or file the item touches**,
which is categorical, not semantic.

So: do not cluster the plan. Group it by target, and get the file ownership from
the docs' own path citations, which is what the briefs now do.

### 10.2 Jev inside the script is priced out of most decisions

The engine has no `decide`, so Jev is only reachable through `agent()` — a whole
subagent spawn, not a 300 ms call. Three decisions the workflow could hand it:

| decision | noul | cost |
| --- | --- | --- |
| should this phase be skipped? | 0.77 | ~295 tok + a spawn |
| retry or escalate after a `partial`? | 0.58 | ~295 tok + a spawn |
| which crates to verify? | 0.37 | ~295 tok + a spawn |

And the workflow's agents already return a structured `status` enum, so most
decisions are a `if status == "blocked"` away.

**Where it could earn a spawn:** the triage of a `partial`. That is the one outcome
the workflow cannot classify with a branch — "partial" covers both "nearly done,
one more pass" and "hit a wall", and those want opposite responses. A noul at 0.58
is not confident about it, which is itself the signal that a branch would be wrong.

**What would change this:** `decide` (§8.1). At 300 ms and no spawn, the price
collapses and all three decisions become worth asking. Until then, the honest
answer is that a `decide`-less workflow should decide with branches.

## 11. Verifying that the workflow covers the plan

`verify_plan_coverage.py` reads every numbered item in `PLAN.md` and every brief in
the script, and asks a judge whether the brief instructs the agent to do the item.
Two-stage, like everything else: retrieval picks the brief, Jev verifies against it.

**Result: 64 of 64 items covered.** The script and the plan agree.

| threshold | covered | flagged |
| --- | --- | --- |
| 0.30 | **64/64** | 0 |
| 0.40 | 63/64 | 1 |
| 0.50 | 61/64 | 3 |
| 0.60 | 59/64 | 5 |

Three things came out of running it:

**1. The judge is strict on enumerated lists.** At 0.6 it flags five items that are
literally in the brief — `4.11` reads `"/todo-gate on|off|status plus a fire-cap
row, an item-cap row (default 3), a picker toggle, AND the settings rows"`, verbatim.
Four of the five sit in one brief, the one written as a numbered list of ten
surfaces. **A line inside a list reads as less of an instruction than a sentence**,
and the threshold that catches real gaps is ~0.3–0.4, not 0.6 — the same calibration
lesson as everywhere else in these docs.

**2. The first run found two real gaps, and the fix was to name things.** `4.7` and
`4.10` were flagged because the brief said "`/prime`" and "`/goal-verify`" without
saying what each must contain. Rewriting the phase-4 brief to name the content of
every row and status — not just the target — closed them. **A brief that lists
targets is not a brief that specifies work.**

**3. Retrieval cannot recover the plan→brief mapping.** Picking the brief by cosine
against the item gets it wrong for **28 of 64 items (43.8%)** — items from the skills
group match `correctness`, items from memory match `phase0`. The mapping is known by
construction, so the verifier should use it and let retrieval only rank *within* a
brief. Same finding as §10.1, one level up: short items against long briefs give a
similarity range with no signal in it.

**What the verifier is not:** it checks that the brief *tells* the agent to do the
item, not that the agent will succeed. And it reads one brief per item — the
intended one — so a brief that over-reaches is invisible.

## 12. Design

| stage | attachment | fail-open |
| --- | --- | --- |
| catalog | append the registry's names and `when_to_use` to the `workflow` tool description | no catalog (today) |
| naming | one `choice` over candidate names the author generates, rules in the question | the author names it |
| model call | **new host fn** — see §7.1 | `agent()` fallback |
| agent_type validation | extend the `validate_only` canned host to resolve types | today: fails mid-run |
| skills binding | `skills_hint` on `AgentOpts` | no binding |

### 11.1 The engine change that matters most

A `decide(state, questions)` host function would let a script ask Jev directly —
one call, ~300 ms, typed answers — instead of spawning a subagent to get a
yes/no. That is the difference between a workflow that branches on a judgement in
300 ms and one that spends a whole subagent on it.

It is also the only way Jev enters a workflow at all: the engine has no model
path today, and `agent()` is the expensive stand-in.

### 11.2 Where Jev does *not* help

- **Writing the script.** Rhai is text; Jev does not generate strings. It picks
  and scores; the author writes.
- **Deciding whether to run a workflow.** With one built-in and one project file
  there is nothing to disambiguate yet; revisit when the catalog grows.

## 13. TUI configuration

**What exists.** `/workflows` lists runs; `/workflow pause|resume|stop <name>`
drives them. `workflows` is a config bool resolved in `agent/config.rs:3160` with
the usual env/remote chain.

**What is missing.**

| recommendation | surface today | needed |
| --- | --- | --- |
| catalog visibility | nothing — not even in the prompt | the tool description first, then a `/workflows` section |
| workflow on/off | a config bool, no row | a settings row |
| `decide` host fn | does not exist | a capability toggle once it does |
| `create-workflow` skill | referenced, not installed | ship it, or drop the reference |

**Shape, following `/jev`.** `/jev on|off|status` dispatches a typed action, the
same one the settings row dispatches; the value persists and the live session
picks it up through the config reload fan-out.

```
/workflows list            the catalog: names, when_to_use, source (builtin/file)
/workflows on|off          the existing config bool, reachable
/workflow-decide on|off    the host fn, once it exists

settings row: "Workflows" bool beside the other agent rows
```

**`/workflows list` must scroll and filter** — the catalog grows and the modal is
already long; use `list_pane` and the `/` filter the other six list modals use
(`FINDINGS.md`, cross-cutting).

`/workflows list` is the cheapest visible fix: it makes the catalog inspectable
where the runs already are, and it is the same data the tool description needs.

## 14. Implementation order

1. **Build the selection as retrieval + Jev, not a listing** (§4). Same accuracy,
   677 tokens per turn cheaper, and it scales when the catalog does.
2. **Append a catalog to the tool description only as a fallback** — it is what
   makes a workflow discoverable at all today, and it is worth keeping until the
   retrieval path exists.
3. **Validate `agent_type` in `validate_only`.** A typo currently passes the smoke
   check and fails mid-run; this is the cheapest correctness fix in the doc.
4. **Add `skills_hint` to `AgentOpts`** so a step can bind a skill, mirroring
   `spawn_subagent`.
5. **Use `agent_type` in the built-in** — it writes it zero times today.
6. **Ship or drop the `create-workflow` skill.** The tool tells the model to read
   a file that is not there.
7. **Add `decide(state, questions)` to the engine.** The only path for Jev inside
   a workflow, and it replaces an `agent()` spawn with a 300 ms call.
8. **Then, and only then, wire naming.** 40% at four candidates is real but small
   next to the discovery gap and the missing host fn.
9. **`/workflows list`** so the catalog is inspectable.

## 15. Hazards

- **A workflow is a file the user may have written.** Any catalog must be built
  from what is actually on disk each session, never cached across a config change
  — the watcher already reloads `.grok/workflows/`.
- **`meta` must be the first statement and a pure literal.** If naming ever moves
  into the script body, the literal requirement constrains it.
- **The engine has no cancellation-friendly model path.** A `decide` call must
  respect the run's pause/stop, or a paused workflow keeps a request in flight.
- **`agent_type` is a literal the compiler never sees.** It is validated at spawn,
  and `validate_only` uses a canned host that never resolves one — so the smoke
  check that is supposed to catch authoring mistakes passes a typo. Any future
  host fn must not repeat this: validate literals where the author can still fix
  them.
- **A brief that lists targets is not a brief that specifies work.** The verifier
  flagged `/prime` and `/goal-verify` until the brief said what each must contain.
  Name the content, not the surface.
- **Short text does not cluster.** Thirty-two one-line titles gave one cluster of
  29; the categorical signal (which crate an item touches) beat the semantic one.
  Reach for embeddings when the corpus has spread, not by default.
- **A parallel fan-out needs declared file ownership.** Four of six pairs collided
  on two files before the briefs said who owned what, and a concurrent edit is a
  silent lost write, not an error.
- **The tmux session is state.** A run that leaves `grok-dev` alive holds the dev
  profile for the next one; kill it in the brief and verify it separately.
- **Naming at 40% is not a lever.** Do not spend on it before the catalog and the
  host fn land.
- **A partial listing is worse than none** (5/24). If a listing ships, ship all of
  it — truncating to save tokens loses more than it saves.
- **Retrieval needs the catalog indexed.** `deep-research` is `include_str!` and
  the rest are files; the index must be rebuilt from disk per session, since the
  watcher already reloads `.grok/workflows/`.

## 16. Not verified

- The discovery arms are 8 cases and the blind arm measures recognition, not
  generation — today's real number is worse than 5/8.
- Naming is 15 cases; 40% against 25% chance is one sample.
- The catalog in §3 is invented (four plausible workflows); the real one has one
  built-in and one project file.
- The engine was read, never executed; `decide` is a proposal, not a measurement.
- The `agent_type` authoring arms are 25 cases, and the blind arm measures
  recognition; the agent graph was not tested in the workflow framing.
- The clustering used a naive single-link merge, so "embedding cannot group this"
  is about the item titles and this method, not about clustering in general.
- The Jev prices are one call each on a two-line state; the spawn cost that
  dominates in a real run is not in the token count.
- The collision surface was computed from the file paths each findings doc cites,
  not from a real concurrent run: two agents editing one file has not been observed
  here.
- The tmux phase was written against the repository's documented procedure and never
  executed; the console was not driven in this analysis.
- The `create-workflow` skill was checked in `~/.grok/skills` only; it may exist
  in the repo and ship by another route.
- Nothing was measured inside the TUI.