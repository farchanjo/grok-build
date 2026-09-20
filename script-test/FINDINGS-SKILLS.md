# Skills with Jev — complete findings and design

Consolidated 2026-09-19 from every skills experiment in this folder. Companion to
`FINDINGS-MEMORY.md`; `FINDINGS.md` covers permission, agents and workflow.

Everything measured against `jev-1.13`, native transport, live dev profile
read-only. No Rust was changed. Where a number comes from reading code rather
than running something, it says so.

## 1. Goal

Make skill selection good, make it measurable, and make the injection
intelligent: inject the right skill, inject few, and stay quiet when nothing is
needed.

## 2. What exists today (verified)

| piece | where | behaviour |
| --- | --- | --- |
| selection | `session/prime/skills.rs` | deterministic evidence, then FTS + KNN + weighted RRF, then a shortlist rerank |
| injection | `session/acp_session_impl/turn.rs:977` | `ConversationItem::system_reminder(rendered.text)`, prepended to the user message at `:1828` |
| accounting | `turn.rs:1141` | `LastPrimeOutcome` with `primed_skill_names`, `injected_chars`, `injected_tokens`, `degradation`, `status` |
| projection | `acp_session.rs:748` → `session_setup.rs:625` | `to_projection() -> PrimeContextInfo`, consumed by `scrollback/blocks/context_info.rs` (the `/context` view) |
| prime config | `~/.grokdev/config.toml` | `[prime.skills] enabled`, `max_results = 3`, `max_body_chars = 2000`, `max_total_chars = 6000`, `deadline_ms = 3000`, `degrade_on_error = true` |
| settings UI | `/retrieval-settings` → `retrieval_settings_modal.rs` | edits `prime.skills.enabled` and every field above |
| advertisement | `skill_discovery_tracker/listing.rs` | `SKILL_BUDGET_CONTEXT_PERCENT = 0.5`, `MAX_LISTING_COMBINED_BYTES = 400`, `MIN_DESC_LENGTH = 20` |
| scrollback gate | `acp/tracker.rs:1473` | `user_message_hidden_from_scrollback`, three suppression paths |
| validation | `skills/strict/validator.rs` | pinned to `agentskills` rev `69ef37e9`; invalid skills quarantined; description ≤ 1024 |
| eval runner | `skills/strict/evals.rs` | offline and deterministic: no network, no embedding, no model |
| eval corpus | `~/.grok/skills/*/evals/cases.yaml` | 171 files, every case `resource` or `explicit_pin` |

Roster: 172 skills, description median 470 chars, max 699.

## 3. The defect that explains the empty corpus

`LocalSkillEvidence::matches_query` (evals.rs:466) passes when the lowercased
query is a **literal substring** of the skill's name, description, when-to-use or
short description. `should_trigger` = named AND matches; `should_not_trigger` =
named AND NOT matches. Nothing else.

Against 54 real natural-language requests, same candidate base:

| metric | substring matcher | Jev (noul) | Jev (one choice) |
| --- | --- | --- | --- |
| gold accepted | **0/54** | 29/54 | **39/54** |
| correctly silent on no-skill requests | 12/12 | 12/12 | 10/12 |
| false silences (a skill existed) | **54/54** | 2/54 | **0/54** |

**Every natural-language `should_trigger` case you write today fails.** A
Portuguese sentence never appears verbatim in an English description. That is why
`EvalCaseKind::{ShouldTrigger, ShouldNotTrigger, Conflict}` sit unused.

## 4. The advertisement: the curve saturates at 200

`MAX_LISTING_COMBINED_BYTES = 400` per entry; 140 of 172 descriptions (81%)
exceed it. The listing costs ~69 KB, ~17k tokens, every turn.

**Isolated** (wide-roster choice, no shortlist):

| index width | top-1 | gold in top-10 | tokens |
| --- | --- | --- | --- |
| 60 | 77.8% | 94.4% | 364k |
| 200 | 75.9% | 98.1% | 805k |
| 400 (what ships) | 70.4% | 98.1% | 1.43M |
| 700 (full) | 66.7% | 94.4% | 1.74M |

**In the stacked pipeline** (shortlist choice, single-noul gate, variants
routing, no body re-read) the picture inverts and then saturates. Advertisement
cost is `172 x width` bytes per turn; selection cost is `10 x width`, so the
first dominates 17x:

| width | top-1 | MRR | advertisement tok/turn | tok per extra hit |
| --- | --- | --- | --- | --- |
| 60 | 55/66 | 0.725 | 2,580 | — |
| 100 | 56/66 | 0.735 | 4,300 | 1,825 |
| 150 | 56/66 | 0.738 | 6,450 | 4,090 |
| **200** | **57/66** | 0.748 | 8,600 | 3,178 |
| 300 | 57/66 | 0.753 | 12,900 | 5,448 |
| 400 | 57/66 | 0.748 | 17,200 | 7,696 |
| 700 | 57/66 | 0.748 | 30,100 | 14,230 |

**Accuracy saturates at 200 and never rises again.** The shipped 400 pays twice
the tokens — 8,600 per turn — for the same 57/66. The isolated 60-byte result came
from a wide-roster choice that no longer exists once a shortlist does.

**Why 60 is the floor.** The byte at which each lookalike child's distinguishing
token (`bind9`, `sevenz`, `containerd`) appears in its own description, across 58
umbrella/child pairs:

| width | discriminator absent or beyond |
| --- | --- |
| 40 | **55/58 (95%)** |
| 60 | 2/58 (3%) |
| 100 | 2/58 (3%) |

The token lands at byte ~55. Below 40 breaks almost everything.

**Cleaning the scaffolding** is neutral at 400 (same accuracy, −171 selection
tokens) and worth +1 at 60 (55 against 54 raw). Keep it: free either way.

## 5. The corpus is written for a keyword matcher

Descriptions carry inline `TRIGGER: ...` and `SKIP: ... (use X instead)` sections
and a `metadata.triggers` keyword list. Umbrella skills also declare
`metadata.variants`, and say so in prose: *"Hub stub never inlines variant
content; matcher picks highest-scoring variant per current context."*

- 27 skills declare `variants`. Nothing reads the field.
- 18 skill names are a hyphen-subset of another and absorb 58 children between
  them (`spring` 6, `archive`/`container`/`dns`/`logging`/`metrics` 4 each).
- 0 of 172 skills use `when_to_use`, so the 400 bytes are never split.
- 0 descriptions start with a trigger prefix, so `strip_leading_trigger_prefix`
  never fires on this corpus.
- 2 children never name their own discriminator at any width (`sevenz-archive`
  says "7z", never "sevenz").

**Using the declared taxonomy works.** When the chooser lands on an umbrella that
declares variants, asking a second question over exactly those children:

| arm | top-1 | top-3 | MRR | routed | fixed | broke |
| --- | --- | --- | --- | --- | --- | --- |
| choice only | 44/66 | 48/66 | 0.706 | — | — | — |
| **choice + variants routing** | **46/66** | **51/66** | **0.736** | 8 | 3 | 1 |

Net +2. It fires only when the umbrella wins and declares children, and it has a
false-positive mode: when the umbrella *is* the right answer (`gold=dns`), routing
pushes it down to a child. A confidence floor on the second answer would blunt it.

## 6. The gate: saturates as a mean, works as one noul

Mean of three nouls at threshold 0.30 compresses: 56 of 66 cases landed in the
0.4–0.6 band where a skill exists 100% of the time, and it fired "no skill" on
only 2 of 12 uncovered cases.

One noul at 0.40 is the working shape, and in the stacked ablation reverting to
the mean costs **4 top-1** (51 against 55) while correct silence on no-skill
requests drops from 11/12 to 8/12. It is the single largest contributor measured.

## 7. The body re-read hurts

The cookbook's second stage (re-read the top three with full bodies) rescued 3 and
broke 9. The umbrella body claims the whole area, so more text makes the umbrella
win more. This roster has an umbrella layer; the cookbook's does not.

In the stack it costs −1 and **doubles the tokens** (103k against 56k).

## 8. Retrieval space

Only solaris models. `native` is the no-model lexical path.

| # | retrieve | embed | rerank | jev | top-1 | top-10 |
| --- | --- | --- | --- | --- | --- | --- |
| **1** | **hybrid** | **solaris** | **none** | **on** | **42/66** | 54/66 |
| 2 | hybrid | solaris | real | on | 41/66 | 54/66 |
| 3 | native | — | none | on | 35/66 | 39/66 |
| 4 | native | — | real | on | 34/66 | 39/66 |
| 5 | hybrid | solaris | none | off | 26/66 | 54/66 |
| 6 | native | — | real | off | 26/66 | 39/66 |
| 7 | hybrid | solaris | real | off | 25/66 | 54/66 |
| 8 | native | — | none | off | 20/66 | 39/66 |

- **Jev dominates**: +16 on hybrid, +15 on native.
- **The solaris embedding sets the ceiling**: top-10 reaches 54 with it, 39 without.
- **The reranker only pays where retrieval is weak**: +6 on native, −1 on hybrid.
  It substitutes for retrieval quality rather than adding to it.
- `native + jev` at 35/66 is the fail-open floor when the embedding is down.

## 9. Intelligent injection

Selection accuracy is not the whole question once the chosen skill is injected —
its body enters the context and stays. The metric that matters is precision at a
fixed budget, plus how often the injector declines.

Six injectors, same 66 queries, same 3-skill budget, same retrieval:

| injector | injects/query | precision | gold injected | gold first | harmful (12 no-skill) | injected body tok/turn |
| --- | --- | --- | --- | --- | --- | --- |
| native top3 | 2.36 | 19.9% | 31/54 | 20/54 | 2/12 | 1,181 |
| milvus top3 | 3.00 | 23.2% | **46/54** | 32/54 | 12/12 | 1,500 |
| hybrid top3 | 3.00 | 21.7% | 43/54 | 26/54 | 12/12 | 1,500 |
| jev gate + choice | 0.80 | 79.2% | 42/54 | 42/54 | **1/12** | **401** |
| **hybrid + jev** | **0.79** | **86.5%** | **45/54** | **45/54** | **1/12** | **393** |
| hybrid + jev scored | 2.20 | 31.7% | **46/54** | 43/54 | 1/12 | 1,098 |

- **The blind injectors are bad.** Precision 20–23%: they inject three skills and
  under one is right. They also inject on **12 of 12** no-skill requests.
- **The gate is what makes it intelligent.** Harmful injection drops from 12/12 to
  1/12, precision rises from 21.7% to 86.5%.
- **Cost falls with it**: 393 injected tokens per turn against 1,500 — 4x less
  context occupied. `hybrid + jev` beats `hybrid top3` on every column, recall
  included, so it is not a trade.
- **The embedding feeds, the gate filters.** `milvus top3` has the best recall
  among the blind injectors (46/54); `hybrid + jev` reaches 45/54 on a quarter of
  the context. `jev gate + choice` alone, over the whole roster with no embedding,
  reaches 42/54 — the embedding adds +3 on top.
- **The scored variant is the recall play**: 46/54, the best of all, at 2.2 skills
  and 1,098 tokens per turn.

**Recommendation: gate, then one choice over the shortlist.** Under one skill per
turn on average, right 86.5% of the time, and quiet when nothing is needed.
Milvus is not an alternative to Jev — it feeds the shortlist; Jev is what stops
Milvus from dumping three skills into every request.

## 10. The injection is also the persistence, and it is invisible

### 10.1 It persists

`turn.rs:1828` puts the reminder into `items` and it crosses the persistence
barrier with the user message. The single API conversion point
(`conversation.rs:2406`) does not filter `synthetic_reason` — only folds
`Reasoning`. So an injected skill is re-sent on every later request until
compaction drops it, at `max_total_chars = 6000` per injection: ten primed turns
accumulate ~60 KB, ~15k tokens, all re-sent.

Prime reuses the generic `SyntheticReason::SystemReminder`, shared with ten other
origins, so nothing can tell prime's reminder apart at build time.

Cache note, from the repo itself (`subagent/mod.rs:1450`): stripping synthetics
"would diverge the child prefix at the first removed item and cap radix reuse
there". But prime's content is derived from the current prompt and is already
intermittent (`turn.rs:975`), so the prefix already diverges.

### 10.2 Nothing shows it

Two independent layers, and fixing one alone changes nothing:

1. **The reminder is never announced to the UI.** The emission loop at
   `turn.rs:1834` iterates `prompt_blocks` — the *user's* blocks — while the
   reminder only lives in `items`.
2. **Even emitted, the scrollback would drop it.**
   `user_message_hidden_from_scrollback` (`acp/tracker.rs:1473`) suppresses any
   user message starting with `<system-reminder>`. The test at `tracker.rs:6560`
   names it "legacy system-reminder still suppressed".

The one surface that exists is `/context`, rendering `status`, `primed_skill_names`,
`injected_chars`, `injected_tokens` and `degradation` — only for the last eligible
turn, only if opened, only when `should_render()` holds.

### 10.3 The toggle already exists

`/retrieval-settings` opens `retrieval_settings_modal.rs`, which edits
`prime.skills.enabled` and every field. What is missing is a fast toggle.

### 10.4 Design: visibility first, then the strip

1. **`SyntheticReason::SkillPrime`** — prerequisite for filtering and for
   distinct rendering.
2. **Emit a marker** when prime injects, carrying the selected names and token
   cost — all already in `LastPrimeOutcome`. One line, e.g.
   `◆ primed: dns, bind9-dns (1.2k tokens)`.
3. **Then strip at request build**, keyed on `SkillPrime`. The marker keeps the
   transcript honest about what was sent.
4. **Fast toggle** `/prime on|off|status`, `/retrieval-settings` for the rest.
5. **Status-bar segment while primed**, cleared when off.

## 11. Design

| stage | attachment | fail-open |
| --- | --- | --- |
| advertisement width | `MAX_LISTING_COMBINED_BYTES` 400 → 200 | unchanged |
| description cleaning | strip `TRIGGER:`/`SKIP:` at listing time | unchanged |
| single-noul gate | replaces the three-noul mean, threshold 0.40 | prime's gate |
| selection | one `choice` over the shortlist, gate first | RRF order |
| taxonomy routing | second `choice` over `metadata.variants` when the pick is an umbrella | keep the umbrella |
| eval runner | opt-in semantic arm for `should_trigger` | offline runner unchanged |
| injection | `SyntheticReason::SkillPrime` + filter at request build | today's behaviour |
| visibility | marker chunk on inject, driven by `LastPrimeOutcome` | today: nothing |
| control | `/prime on\|off\|status` beside `/retrieval-settings` | config unchanged |
| status bar | segment while primed, cleared when off | no segment |

Question shapes that measured well:

```
gate:     one noul, "Would a careful expert answering this request consult a
          specific documented procedure or set of commands?", threshold >= 0.40
select:   one choice over the shortlist, index at 200 bytes, cleaned,
          "When a broad umbrella skill and a specific one both cover the request,
           prefer the specific one."
variants: one choice over the umbrella's declared children only
fit:      one noul per candidate when recall matters more than context
```

## 12. TUI configuration

**What exists.** `/retrieval-settings` (aliases `/retrieval`,
`/retrieval-config`) opens `retrieval_settings_modal.rs`, which edits
`prime.skills.enabled` and every field of `SkillPrimeConfig` — `max_results`,
`max_body_chars`, `max_total_chars`, `max_tokens`, `max_context_factor`,
`deadline_ms`, `degrade_on_error`, `min_score`. A settings row
(`open_retrieval_settings`) deep-links to it. That is a complete config surface.

**What is missing.**

| recommendation | surface today | needed |
| --- | --- | --- |
| index width 200 | compiled constant `MAX_LISTING_COMBINED_BYTES` | a row |
| cleaning the scaffolding | compiled | a row |
| injection on/off | only by disabling prime entirely | a separate toggle |
| injection visibility | nothing in the transcript (§10.2) | the marker |
| fast toggle | `/retrieval-settings` is a modal, not a toggle | `/prime on\|off` |

**Shape, following `/jev`.** `/jev on|off|status` dispatches a typed
`Action::SetCompactionJevEnabled`, the same action the settings row dispatches,
so the slash command and the row cannot drift; the value persists and the live
session picks it up through the config reload fan-out. `status` reports the
effective value.

```
/prime on|off|status        toggles injection only; selection config stays in
                            /retrieval-settings
  status  -> enabled, index width, last primed names and injected tokens
             (all already in LastPrimeOutcome, §2)

settings rows: "Skill injection" bool, "Skill index width" int, and
               "Strip TRIGGER/SKIP scaffolding" bool (default on) — all beside
               the existing prime rows in the retrieval modal
```

The `status` line is the cheap half of the visibility problem: it reuses
`PrimeContextInfo`, which already renders in `/context`.

## 13. Implementation order

Measured on the stacked pipeline with one ablation per item, so each contribution
is visible on top of everything else rather than in isolation. Starting point is
the shipped shape at 39/66; the corrected stack reaches 55/66 with 94k fewer
tokens.

1. **Single-noul gate at 0.40.** Biggest single contributor: reverting to the
   three-noul mean costs 4 top-1 and drops correct silence from 11/12 to 8/12.
2. **Wire `metadata.variants`** into selection. −2 when removed, −3 on top-3.
3. **Drop the body re-read.** −1 when added, and it doubles the tokens.
4. **Set the index to 200 bytes and clean the scaffolding.** Accuracy saturates
   at 200; the shipped 400 pays 8,600 tokens per turn for nothing.
5. **Make the injection a gate plus one choice** (§9). Precision 86.5% against
   21.7%, a quarter of the injected context, and 1 harmful injection against 12.
6. **Write NL `should_trigger` cases** for the top ~40 skills; the 54-case set
   here is the seed. They all fail today, which is the point.
7. **Semantic eval arm**, opt-in, behind the offline runner.
8. **`SyntheticReason::SkillPrime`**, then the marker chunk, then the strip
   (§10.4). In that order: the strip is only safe once the injection is visible.
9. **`/prime on|off|status`** and the status-bar segment.
10. Point the embedding at `192.168.200.32:8001`; leave `reranker_models` empty.

**Not in the critical path.** The specificity hint and description cleaning are
neutral in the stacked pipeline (identical MRR to four decimals). They were worth
+7 and +1 against a wide-roster choice, which the shortlist removes. Keep cleaning
because it is free; treat the hint as optional.

## 14. Hazards

- Narrowing changes what the model sees; a skill whose discriminator sits past
  byte 60 becomes invisible. Re-run the discriminator check per skill.
- Variants routing has a false-positive mode when the umbrella is correct. A
  confidence floor on the second answer would blunt it.
- The specificity hint was measured on one roster with deliberate lookalikes.
- `should_not_trigger` cases matter as much as positive ones: 12 of the 54 cases
  exist to punish guessing.
- **Injecting has an asymmetric cost.** A wrong injection occupies context and
  steers the model; a missed one only loses help. Bias toward declining.
- **The tracker has three suppression paths.** Touching one and missing the
  others gives different behaviour on old and new sessions.
- **The marker must stay one line.** A marker that inlines the body rebuilds the
  problem it solves.
- **Stripping makes the transcript differ from what was sent.** The marker is the
  mitigation, so the two ship together.

## 15. Not verified

- One run per arm; differences of one query do not separate.
- The 66-case set is mine, hand-labelled, with hard lookalikes.
- Prime was not executed; the pipeline here is a faithful core, and the native
  arm is FTS5 alone — the real prime also scores deterministic evidence.
- The matcher was ported by hand, not run in Rust.
- Cache impact of stripping was reasoned from comments, not measured.
- Advertisement cost is computed (`172 x width / 4`); only selection cost is
  measured.
- Nothing was measured inside the TUI.
- The visibility findings come from reading code paths, not from a live session
  with `RUST_LOG` on to confirm the reminder is absent from the UI.
- How `scrollback/blocks` would render a marker was not designed.
- The `/retrieval-settings` modal was read, not driven.