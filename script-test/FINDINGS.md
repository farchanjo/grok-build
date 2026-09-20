# Where Jev fits the harness — findings

Measured 2026-09-19 against `jev-1.13`, native transport, one run per arm.
Everything below is from a simulation in this folder; no Rust was changed.

This is the umbrella doc. Three targets have their own, much longer references and
are summarised here: **`FINDINGS-PERMISSION.md`** (auto-mode, the floors, the
residual), **`FINDINGS-SKILLS.md`** (selection, injection, eval runner,
visibility) and **`FINDINGS-MEMORY.md`** (scoping, admission, coexistence) and
**`FINDINGS-AGENTS.md`** (recommendation, selection, effort) and
**`FINDINGS-GOAL.md`** (goal verification, the skeptic panel) and
**`FINDINGS-TODO-GATE.md`** (the turn-end backstop) and
**`FINDINGS-WORKFLOW.md`** (authoring, discovery, the Rhai engine) and
**`FINDINGS-TOOLS.md`** (tool search) and **`FINDINGS-JEV-TRANSPORT.md`**
(transport selection, the session-id hazard).
`FINDINGS-vs-stack.md` covers skill selection against the retrieval stack.

**Naming.** The LAN box is `solaris` (`192.168.200.32`). Earlier scripts labelled
its models "local", which reads as "on the Mac" and is wrong.

## The inventory: every decision seam that takes a judgement

The sweep found more than the four targets. Ordered by how much the harness
already leans on a fragile judgement.

| seam | what decides today | fit | why |
| --- | --- | --- | --- |
| **permission auto-mode** (`permission/auto_mode.rs`, 3169 lines) | LLM classifier with a huge prompt, plus a fail-closed heuristic fast-path and tree-sitter shell parsing | **high, but not where you think** | the classifier is already tuned (precision 0.94, recall 1.00 held-out on 567 real commands); **four deterministic floors override its Allow**, and 44% of policy-allows are prompted regardless — see `FINDINGS-PERMISSION.md` |
| **laziness / stop detector** (`laziness_classifier.rs`) | LLM emitting `{category, confidence, evidence}` as strict JSON | **high, with one gap** | `min_confidence` already exists, so calibration slots in; but the contract needs a free-text `evidence`, which Jev cannot produce |
| **skill selection and injection** (`prime/skills.rs`) | deterministic retrieval + optional rerank, evaluated only by substring | **high** | detailed in `FINDINGS-SKILLS.md`; Jev is worth +16 top-1 and the injection gate is the single largest measured contributor |
| **memory keep/drop, scope, rerank** | nothing for admission; everything drifts to global | **high, additive** | detailed in `FINDINGS-MEMORY.md`; 100% precision/recall with the store in state, scope 82.4%, rerank 9/10 |
| **agent recommendation and selection** (`prime/agents.rs`) | deterministic ranking, advisory-only, revalidated against live authority | **high, for a different reason** | detailed in `FINDINGS-AGENTS.md`; the advisory block buys +1 case, the effort question is worse than a constant, dense retrieval is worth +8 over lexical, and the unused `## Delegates` graph is worth about one case once a retriever exists |
| **goal verification** (`goal_classifier.rs`, 6612 lines) | **not a classifier** — a panel of 3 adversarial skeptic subagents, strict majority, veto gatekeeper, fail-closed, up to 10 runs | **low, as a pre-filter only** | detailed in `FINDINGS-GOAL.md`; Jev scores 67% and its errors are exactly the prompt's non-naive rules, while the required `evidence` field is text it cannot produce |
| **tool search** (`session/tool_index.rs`) | BM25 over the MCP toolset, exact-name short-circuit, index rebuilt per call | **high** | detailed in `FINDINGS-TOOLS.md`; BM25 finds the gold in the pool **7 of 35** times cross-lingually against **33 of 35** for the embedding, and the `ToolSearchIndex` trait is an existing seam |
| **notification admission** (`tools/notification_bridge.rs`) | token bucket plus a two-value producer-set priority | **low** | `admit()` is a throttle, not a judgement; `NotificationPriority::{Next, Later}` is set at the call site. Never analysed in depth — no doc |
| **workflow step roles** (`AgentOpts.agent_type`, `host_service.rs:459`) | a literal string, unvalidated until spawn, `general-purpose` default; no `skills_hint` | **medium** | the built-in uses it zero times and re-describes roles in prose; a typo passes `validate_only` (canned host) and fails mid-run — `FINDINGS-WORKFLOW.md` §6 |
| **workflow authoring and discovery** (`xai-workflow/src/engine.rs`, `session/workflow/registry.rs`) | Rhai host with **no model call**; the tool description never lists the registered workflows | **medium** | detailed in `FINDINGS-WORKFLOW.md`; retrieval + Jev matches a full listing at **zero per-turn cost**, and a `decide` host fn is the only way Jev enters a script |
| **todo gate** (`reminders.rs`) | four-line deterministic branch; no model | **medium, narrow** | detailed in `FINDINGS-TODO-GATE.md`; the gate is finished and **off by default**, and the one judgement left — which item to advance, since "next" is insertion order — scores 61% by insertion and 83% with Jev at zero losses |
| **compaction pruning** | Jev | — | shipped |

Two structural findings from the sweep:

- **The skill eval corpus is empty where it matters.** 171 skills ship
  `evals/cases.yaml`, but every case is `resource` or `explicit_pin` — 342
  mechanical cases, **zero natural-language ones**. The reason is measured in
  `FINDINGS-SKILLS.md` §3: the runner matches literal substrings, so a real
  request never matches and every NL case would fail. `EvalCaseKind` already
  declares `should_trigger` and `should_not_trigger`; nothing uses them.
- **`trace_classify` already exists** as an offline replay harness for the todo
  gate and the laziness classifier, emitting what each would have decided per
  turn. That is the natural place to A/B Jev against the production classifier
  on real traces, without touching the live path.

## Measured: the permission gate

Summary only; `FINDINGS-PERMISSION.md` has the full set. The headline is that
**the decision does not live in the classifier.**

The gate is layered — fast-path, then the classifier, then **floors**, then the
user — and the four floors (`write`, `unsafe_env`, `opaque_shell`, `exec`) each
override a classifier Allow on a structural property of the command.

| measure | n | share |
| --- | --- | --- |
| commands hitting a floor | 63 of 127 | 50% |
| … and that the policy allows | 22 | **44% of all allow** |
| residual that reaches the classifier | 64 | 50% |
| floor tax: both policy and model allow, a floor prompts anyway | 19 | 16 of them the `exec` floor |

On the residual, the errors are **one class**: `npm ci`, `pnpm i`, `uv sync`,
`yarn add` are false blocks; `npx cowsay`, `rustfmt`, `uv tool run ruff` are false
allows. That is a policy disagreement, not a calibration one.

And **the criteria text is the operating knob, stronger than the threshold**:

| criteria | threshold | agreement | false allow | false block |
| --- | --- | --- | --- | --- |
| long | 0.50 | 85.9% | 8% | 21% |
| short | 0.50 | 87.5% | 22% | 0% |
| **short** | **0.60** | **90.6%** | **11%** | **7%** |

Long criteria shift the distribution toward block, short toward allow; the
threshold then picks inside that range. Tuning the threshold alone cannot move
the range.

Four findings from the earlier round survive: a one-call model reproduces ~90% of
a 3169-line policy engine; the prose policy paragraph was the worst arm; labelled
examples buy the dangerous direction at a usability price; and the failures
cluster on wrappers and heredocs (33% and 83% agreement, small n).

## Measured: skill selection and injection

Summary only; `FINDINGS-SKILLS.md` has the full set, including the two results
below that the earlier version of this doc got wrong.

**The stacked pipeline reaches 55/66 against the shipped shape's 39/66, with 94k
fewer tokens.** Contributions measured by ablation, largest first: single-noul
gate (+4), taxonomy routing (+2), dropping the body re-read (+1).

Two corrections the earlier summary carried wrong:

- **The specificity hint is not a +7 lever in the stack.** It is +7 against a
  wide-roster choice and **neutral** once a shortlist exists (identical MRR to
  four decimals). Same for description cleaning: +1 at 60 bytes, neutral at 400.
- **Narrowing the index to 60 bytes is not the win.** Accuracy saturates at 200;
  the shipped 400 pays 8,600 tokens per turn for the same result. Below 40 bytes
  the lookalike discriminator disappears for 95% of pairs.

**Injection is where the quality lives.** Six injectors, same budget of 3 skills:
the blind ones (native, milvus, hybrid top-3) have 20–23% precision and inject on
**12 of 12** no-skill requests. A gate plus one choice reaches 86.5% precision,
injects 0.79 skills per turn instead of 3, and is harmful 1 time in 12.

## Measured: memory

Summary only; `FINDINGS-MEMORY.md` has the full set and the three defects that
must be fixed first.

- **Scope routing** 82.4%, with every error leaning local — the safe direction.
- **Admission and dedup** reach 100% precision and 100% recall with the store in
  the state and the threshold at 0.20. Without the store, the gate re-stores a
  restatement 6 times out of 7.
- **End to end**: the store shrinks 44%, noise drops 10 of 12, restatements 8 → 1,
  and local queries served by a global entry fall from 6 of 7 to 2 of 7.
- **The solaris embedding beats OpenRouter's** on every recall and embeds 4.5x
  faster. The reranker is neutral to negative.

## Cross-cutting rules

Learned the hard way, and they apply to every target:

- **0.5 is the wrong threshold for this model.** A noul answering "is this worth
  it" sits at 0.20–0.40. It appeared four times: skill gate, skill `fits`, memory
  value, memory admission. At 0.5 the memory gate loses 7 of 36 valuable notes.
- **One `noul` per candidate.** A single "does the winner fit" question conflates
  candidates and lands mid-scale; averaging several nouls compresses toward the
  middle and stops firing.
- **Prose added to a question is unreliable.** Five attempts, one partial win: a
  policy paragraph hurt the permission gate, a scope tie-break hurt scope
  routing, four targeted permission rules each made the residual worse, and a
  specificity sentence helped skill routing in isolation before vanishing in the
  stack. Test each, and re-test after the pipeline around it changes.
- **The criteria text is the operating knob, not the threshold.** True/false
  definitions set the range the model works in; the threshold only picks a point
  inside it. Tune them as a pair, and never change one without re-measuring.
- **A list that grows must scroll and search, and the key is `/`.** Six modals already
  do it (`views/memory_modal.rs`, `views/agents_modal.rs`, and four others): `/`
  enters filter mode, Escape exits, filtered indices are cached, and `list_pane`
  virtualises the rows so a 500-row modal costs what a 5-row one does. Every new
  list — settings rows, `/workflows list`, a tool-search status — must reuse that
  rather than hand-roll, and must never truncate to fit. Adding rows to a modal is
  the easiest way to make the console worse, and it is invisible in a diff.
- **The worktree is shared with live sessions, and the tree moves under you.** Another
  session builds, edits and leaves uncommitted work. So: never `git add`/commit,
  never revert someone else's file, re-read before rewriting, never kill a process
  or a tmux session you did not start, and check `ps` again before each build —
  two cargo invocations can be live at once (observed), and the lock only makes
  them queue. `.detent/` and `script-test/` belong to other work.
- **A session-id carrier is added, never replaced.** Six already exist and each is
  gated differently: `x-grok-session-id` (first-party only), `X-Session-ID`
  (self-hosted family only), the body `session_id` + `bootstrap_room`
  (suppressed for OpenAI and Anthropic), OpenRouter's native `session_id`,
  Anthropic's `metadata.user_id`, OpenAI's `prompt_cache_key`. Plus an MCP
  `{{session_id}}` header expansion that **drops the header silently** when there
  is no session. A new caller derives its own key; it never reuses the session's.
- **A cache key is a routing key.** The session id feeds `bootstrap_room`
  (`fnv1a_32`), `prompt_cache_key` and `metadata.user_id` across six providers. A
  new caller that reuses the session's own key claims a prefix family it does not
  belong to; a derived key costs nothing and cannot regress the main path.
- **A lexical retriever is monolingual, and that is a recall cliff.** The tool
  search returns an empty pool for 28 of 35 Portuguese requests against English
  descriptions; the skill eval matcher scored 0 of 54 on the same mismatch. Both
  are the same defect in different subsystems. Check the language of a corpus
  against the language of its queries before trusting a lexical ranker.
- **A listing is a per-turn cost; retrieval is a per-call one.** The workflow
  catalog costs 677 tokens every turn whether a workflow runs or not; the same
  accuracy came from retrieval at zero per-turn cost. When a catalog is small a
  listing is fine — measure the crossover rather than assuming either.
- **A capability the prompt cannot see is a capability the model cannot use.**
  The workflow registry holds the catalog and the tool description never mentions
  it; the `create-workflow` skill is referenced and not installed. Both are static
  text, and both were invisible until the description was read against the code.
- **Sometimes the TUI work is the whole work.** The todo gate is finished, correct
  and deterministic — and disabled, for a reason that is purely UI plumbing
  (`settings/defs.rs:1943` documents it). Check whether a feature is off because
  it is bad or because nobody built the switch.
- **A long prompt is a mechanism, not verbosity.** The goal verifier's prompt is
  thousands of words of exceptions and anti-naive rules; Jev reproduces the naive
  reading and fails exactly on those rules (67%). Porting the prompt to a short
  version would keep the naive reading and lose the exceptions.
- **Two corpora, two readers.** The skill descriptions are written for a keyword
  matcher (inline `TRIGGER:` lists, the keyword repeated verbatim); the agent
  descriptions are written for a human (`C++`, not `cpp`). A substring matcher
  scores the second badly and the first well; a semantic chooser is the reverse.
  Check which reader a corpus was written for before choosing a mechanism.
- **Look for declared structure nothing reads.** Skills carry `metadata.variants`
  and agents carry `## Delegates`; both are hand-authored routing, both unread.
  The TUI is the same story: `list_pane` and `picker` already ship virtualised
  scroll and a search bar, and six modals use them — check for the component
  before building one.
  Grep for the field before designing a new mechanism — but measure it *in the
  full stack*: the agent graph was +8 against a flat 109-way choice and ~0 once a
  retriever existed, because it substitutes for retrieval.
- **Look for the layer above.** The permission classifier is already well tuned
  and still 44% of what the policy allows gets prompted — by deterministic floors
  it does not control. Measuring the model first would have found nothing.
- **Blind injection is expensive.** Injecting has an asymmetric cost: a wrong
  injection occupies context and steers the model, a missed one only loses help.
  Bias toward declining.

## Negative results worth keeping

- A policy paragraph in the state made the permission gate worse, and so did four
  targeted rules written afterwards — including the package-manager rule, which
  made package managers worse.
- The cookbook's two-stage skill selector loses on this roster: the umbrella body
  claims the whole area, so re-reading bodies makes the umbrella win more
  (rescued 3, broke 9).
- Full skill descriptions cost 4.7x the tokens of a 60-byte index — and in the
  stacked pipeline the wider index wins, so the isolated result does not transfer.
- A cross-encoder reranker placed before Jev never helps in the hybrid path and
  only pays on the lexical path, where retrieval is weak. It substitutes for
  retrieval quality rather than adding to it.
- Tuning the permission threshold alone does nothing useful: at 0.30 the residual
  is 50/64 with 39% false allow, at 0.70 it is 54/64 with 25% false block, and no
  point on the curve beats changing the criteria and the threshold together.

## TUI configuration: the standing requirement

Everything recommended in these docs has to be reachable and toggleable from the
TUI, not only from a config file. That is a requirement on each change, not a
follow-up.

**The pattern already exists and is the one to copy.** `/jev on|off|status`
dispatches a typed `Action::SetCompactionJevEnabled` — the *same* action the
settings row dispatches, so the slash command and the row cannot drift. The value
persists to config and the live session picks it up through the config reload
fan-out, with no restart. `status` reports the effective value. Every target
below mirrors that shape: one typed action, one slash command, one settings row,
one `status`.

| target | surface today | gap |
| --- | --- | --- |
| permission | `permission_mode` setting + Ctrl+O (Default/Ask/Auto/Always) | no classifier toggle, no threshold, no floor telemetry |
| laziness | **nothing** — only `--laziness-debug-log` | the config type exists (`idle_threshold_ms`, `min_confidence`, `max_nudges_per_session`) with no row and no modal |
| skills | `/retrieval-settings` covers `prime.skills.*` fully | no fast toggle, no index-width row, no injection marker |
| memory | `/retrieval-settings` covers mode, vector store, profile | no gate toggle, no thresholds, no scope visibility |
| Jev itself | `/jev` + `compaction_jev_enabled` | the precedent |

`status` is the cheapest visibility win in every case: the data already exists
(`LastPrimeOutcome`, `PrimeContextInfo`, the floor counters, the classifier
category), it is simply not surfaced where the user is.

**Every recommendation needs a row, including the ones that look compiled-in.**
An audit of the seven target docs against their own implementation orders found
five recommendations with no toggle at all: the four permission floors, the todo
gate's item picker, the skill scaffolding strip, memory's master switch and
`save_on_end`, and the agents' delegates graph. A recommendation a user cannot
turn off is a decision made for them.

**A row needs a default that matches what ships.** Every row above defaults to
today's behaviour, so applying the whole set changes nothing until someone flips
one — and each row is independently reversible.

Per-target detail: `FINDINGS-PERMISSION.md` §8, `FINDINGS-LAZINESS.md` §8,
`FINDINGS-SKILLS.md` §12, `FINDINGS-MEMORY.md` §8, `FINDINGS-AGENTS.md` §8,
`FINDINGS-GOAL.md` §6, `FINDINGS-TODO-GATE.md` §6, `FINDINGS-vs-stack.md` §6,
`FINDINGS-WORKFLOW.md` §9, `FINDINGS-TOOLS.md` §5, `FINDINGS-JEV-TRANSPORT.md` §6.

## Recommended order

1. **Permission: shorten the criteria and move the threshold to 0.60**
   (`FINDINGS-PERMISSION.md` §7) — measured 90.6% with 11% / 7%, against 85.9%
   with 8% / 21%. No new machinery.
2. **Instrument the permission floors.** 19 interruptions are invisible today,
   16 of them the `exec` floor. This is what makes the tax arguable.
3. **Skill injection** (`FINDINGS-SKILLS.md` §9) — gate, then one choice. The
   largest measured quality contribution in the whole sweep.
4. **Skill eval cases** — write `should_trigger` / `should_not_trigger` first;
   they all fail today, which is the point.
5. **Memory** (`FINDINGS-MEMORY.md` §7) — after the three defects, since routing
   to global is pointless while global bodies are unindexed.
6. **Laziness** — replay through `trace_classify` before touching production;
   the `evidence` field is the design question.
7. **Porting the permission classifier to Jev is a cost play, not an accuracy
   play.** It is already at precision 0.94 / recall 1.00 held-out.

## What is not verified

- One run per arm, no repetitions, so no variance estimate. The four permission
  rule arms (51–56 of 64) sit inside that band; the criteria/threshold pair does
  not, because it changes the shape of the distribution.
- Permission labels are the policy, not adjudicated truth; a disagreement may be
  the policy being wrong. 16 disagreements were eyeballed, not adjudicated by a
  third party.
- **The permission floors were approximated by regex**, never by running the
  tree-sitter parser, and the corpus assumes no exact grants — which is the one
  thing that disables a floor.
- The production permission classifier was never executed; Jev stood in for it,
  and its precision/recall figures are quoted from its doc comment.
- How much traffic the fast-path absorbs before the classifier is never reached
  was not measured.
- The laziness and goal classifiers were read, not measured: they need traces.
- Skill and memory cases are hand-labelled, not an oracle.
- Arm A of every retrieval comparison is a faithful core, not the production code
  path; prime itself was never executed.
- Cache and latency effects of the proposed changes were reasoned, not measured.
- Nothing was measured inside the TUI.