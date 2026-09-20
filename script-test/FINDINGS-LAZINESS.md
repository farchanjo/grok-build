# Laziness / stop detector with Jev — complete findings and design

Consolidated 2026-09-19. Companion to `FINDINGS-PERMISSION.md`,
`FINDINGS-SKILLS.md` and `FINDINGS-MEMORY.md`; `FINDINGS.md` is the umbrella.

Measured against `jev-1.13`, native transport, live dev profile read-only. No
Rust was changed.

## 1. Goal

Catch an agent that stopped early, without nudging one that stopped correctly.
The cost is asymmetric and both directions are real: a false nudge accuses the
agent of idling and burns a turn, a miss lets work sit unfinished.

## 2. What exists today (verified)

| piece | where | behaviour |
| --- | --- | --- |
| trigger | `LAZINESS_DEFAULT_IDLE_THRESHOLD_MS = 10_000` | fires after 10 s of idle |
| context | `LAZINESS_CONTEXT_ITEM_LIMIT = 30` | at least the last 30 items, extended until per-kind minimums hold |
| classifier | `laziness_classifier.rs` | LLM emitting `{category, confidence, evidence}` as strict JSON |
| categories | `session/events.rs` | four `stalled_*` (`narration`, `permission_asking`, `no_todos_but_task_in_flight`, `false_completion`), three `not_stalled_*` |
| gate | `LAZINESS_DEFAULT_MIN_CONFIDENCE = 0.7` | per-model override via `LazinessDetectorPerModelConfig::min_confidence` |
| nudge | `build_laziness_nudge` | quotes the `evidence` and a `<task_completion_discipline>` rule per category |
| cap | `max_nudges_per_session` | bounds the session |
| replay | `trace_classify` | offline harness, emits what it would have decided per turn |

The prompt is heavily engineered: strict JSON contract, a tamper-proof
`[runtime_state]` line carrying `outstanding_background_tasks_and_subagents` and
`turn_elapsed_seconds`, and an explicit claim-vs-evidence audit ("the agent's
confident prose is NOT evidence — only `[assistant tool_call]` lines count").

## 3. Measured: 27 fixtures

Hand-labelled transcript endings in the shape the prompt documents, 16 stalled
and 11 not-stalled across all seven categories.

| metric | categories passed bare | categories passed with definitions |
| --- | --- | --- |
| category exact | 11/27 (41%) | **20/27 (74%)** |
| … among genuinely stalled | **0/16** | **10/16** |
| … among genuinely not-stalled | 11/11 | 10/11 |

**Passing the category names alone, Jev never once picked a `stalled_*`
category.** Every stalled fixture was labelled `not_stalled_complete`,
`not_stalled_waiting_on_user` or similar — while the separate binary question in
the same request said "stalled" at a median of 0.56. The model knew; the bare
option labels did not tell it where to put the answer.

With the definitions attached, stalled categories go from 0/16 to 10/16. This is
the same lesson as the skill `fits` question: an option label is not a definition.

The confusions that remain are semantic neighbours — `stalled_permission_asking`
↔ `not_stalled_waiting_on_user`, `stalled_false_completion` ↔
`not_stalled_complete` — which are exactly the pairs a reader needs the
claim-vs-evidence rule to separate.

## 4. Measured: the gate is set too high for this model

The binary question's scores, by group: stalled median 0.56 (min 0.08, max 0.79),
not-stalled median 0.21 (min 0.03, max 0.65). The groups separate by median but
their ranges overlap.

| gate | false positive (needless nudge) | false negative (missed stall) | accuracy |
| --- | --- | --- | --- |
| 0.40 | 3/11 | 5/16 | 19/27 |
| 0.50 | 3/11 | 5/16 | 19/27 |
| 0.60 | 3/11 | 11/16 | 13/27 |
| **0.70 (production default)** | **0/11** | **13/16** | 15/27 |
| 0.80 | 0/11 | 16/16 | 11/27 |

**At its own default the detector would fire on 3 of 16 stalls.** 0.7 is
"clearly better than a coin flip" for a chat model's confidence; this model's
nouls sit lower, as they did in the permission, skill and memory gates — the
fifth occurrence of the same calibration mismatch.

Moving to 0.5 trades 3 needless nudges for 8 more catches. Which side of that
trade is right depends on the base rate, and the fixtures are balanced at 59%
stalled while production idle-ends are probably mostly legitimate — so the real
false-positive cost is likely lower than 3/11 and the 0.5 point better than it
looks.

## 5. The `evidence` gap

The nudge embeds `evidence`, a free-text string. Jev returns no text, so a port
keeps the category and the confidence and loses the explanation:

```
"Idle-stall detector flagged this session: {evidence}\n\n{rule}"
```

Options, none measured: drop the evidence clause, keep a parallel text call for
it, or have the harness synthesise it from the category. Dropping it is the only
free option and changes the nudge's tone from "here is what I saw" to "here is
the rule".

## 6. Design

| stage | attachment | fail-open |
| --- | --- | --- |
| category criteria | pass `DEFINITIONS` as the choice criteria | bare labels (today) |
| gate | 0.5 for this model, per-model override unchanged | today's 0.7 |
| evidence | drop, or synthesise from the category | unchanged |
| replay | A/B through `trace_classify` before touching production | new |

```
binary:   one noul, "Is the agent in this transcript STALLED — stranded narration
          with no matching tool call, asking permission for an obvious next step,
          stopping while work clearly remains, or claiming completion without
          tool-call evidence?"
category: one choice over the seven categories, WITH the definitions attached
gate:     0.5
```

## 7. Implementation order

1. **Attach the category definitions.** 0/16 → 10/16 on stalled, no new machinery.
2. **Move the gate to 0.5**, per-model, and measure the false-nudge rate on real
   traffic before committing — the base rate decides the right side of the trade.
3. **Replay through `trace_classify`** against the production classifier, which is
   the comparison this analysis could not make.
4. **Decide the `evidence` clause** last, since it is the only part that cannot be
   replaced by a decision.

## 8. TUI configuration

**What exists.** Almost nothing. `LazinessDetectorPerModelConfig` carries
`max_nudges_per_session`, `idle_threshold_ms` and `min_confidence`
(`config-types/flags.rs:139`), but there is **no settings row and no modal** —
the only surface is the `--laziness-debug-log <path>` CLI flag. So the detector
is on with compiled defaults and cannot be tuned or even inspected without
editing config by hand.

**What is missing.** All of it: on/off, the threshold, the idle window, the nudge
cap, and any visibility of what it decided.

**Shape, following `/jev`.** `/jev on|off|status` dispatches a typed
`Action::SetCompactionJevEnabled`, the same action the settings row dispatches,
so the slash command and the row cannot drift; the value persists to
`[compaction.jev].enabled` and the live session picks it up through the config
reload fan-out, with no restart. `status` reports the effective value.

```
/laziness on|off|status
  on      -> Action::SetLazinessDetectorEnabled(true)
  off     -> no nudges, no classifier call
  status  -> effective state, threshold, idle window, nudges used this session

settings rows (SettingCategory::Agent):
  "Idle-stall detector"     bool, same Action
  "Stall min confidence"    f32, default 0.50 for this model (§4)
  "Idle threshold"          ms, default 10_000
  "Max nudges per session"  u32

status should also print the last decision: category, confidence, whether it
nudged. Today a nudge appears with no way to know what it classified as.
```

The per-model override already exists in config-types, so the rows write to the
same place the flag does — no new resolution path.

## 9. Hazards

- **The base rate decides the gate.** A balanced fixture set flatters a low
  threshold; real idle-ends skew legitimate.
- **`trace_classify` is the right venue.** Production replay already exists and
  emits per-turn decisions; measuring anywhere else repeats this analysis's
  biggest limitation.
- **The `turn_elapsed_seconds` cross-check is where Jev is weakest.** Claims like
  "8 hours overnight" against a few hundred seconds need arithmetic, and the
  prompt asks the model to do it inline.
- **The category is what selects the nudge rule.** A wrong category sends the
  wrong `<task_completion_discipline>` rule even when the binary verdict is right.

## 10. Not verified

- **No baseline.** The production classifier was never run on these fixtures, so
  20/27 is Jev against my labels, not Jev against the incumbent. `trace_classify`
  is how to close that.
- 27 fixtures across 7 categories is 3–4 each; per-category numbers are
  directional.
- Fixtures are hand-written, balanced 59/41 stalled, and in the prompt's
  documented shape — not real transcripts.
- One run per arm.
- The nudge's effect on the agent was not measured: whether a nudged turn
  actually resumes work is a separate question.
- `evaluate_laziness`'s caller and the idle timer were read, not exercised.
- Nothing was measured inside the TUI.