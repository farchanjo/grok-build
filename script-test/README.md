# script-test — Jev efficacy harness

Throwaway experiment folder. Nothing here is needed to build the repository; it
does not touch Cargo, `target-dev`, or anything under `~/.grok` (reads only).

The point is to measure whether TypeSafe's Jev is actually good enough to be a
decision layer in four places, before any Rust is written: skill routing, agent
type and effort, workflow naming, and the memory keep/drop gate.

## Setup

```sh
uv venv --python 3.14 .venv
uv pip install --python .venv/bin/python httpx pydantic numpy rich
.venv/bin/python build_roster.py     # cases/roster.json + cases/agents.json, from ~/.grok
```

Credentials come from `~/.llm-key` (an `export NAME="value"` file), with the
environment taking precedence:

| transport | endpoint | key |
| --- | --- | --- |
| `native` | `api.typesafe.ai/v1/systemone` | `TYPESAFE_API_KEY`, then `TYPESAFE_AI_FABRICIO_KEY` |
| `openrouter` | `openrouter.ai/api/alpha/decisions` | `GROK_JEV_API_KEY`, then `OPENROUTER_API_KEY` |

## Run

```sh
.venv/bin/python run_eval.py --transport native --experiment all
.venv/bin/python analyze.py --transport native      # error breakdown, confusion, calibration
```

Raw calls land in `out/results-<transport>.jsonl`. Case sets are in `cases/`,
every gold label validated against the live rosters before a single call goes out.

## What was measured (2026-09-19, jev-1.13, one run, 202 calls, $0.097)

| experiment | n | accuracy | p50 |
| --- | --- | --- | --- |
| agents (type) | 25 | 80.0% | 333 ms |
| memory (keep/drop gate) | 30 | 73.3% | 289 ms |
| skills, 60-char index | 66 | 68.2% | 339 ms |
| skills, full descriptions | 66 | 60.6% | 467 ms |
| workflow (naming) | 15 | 53.3% | 287 ms |

Both transports returned identical picks on three of five experiment groups and
differed on one case in `skills/short`: same model behind both
(`jev-1.13` / `typesafe/jev-1.13-20260917`). The alpha proxy accepts `choice`
and `score`, not just `noul`.

### Findings that matter

1. **Skill misses are systematic, not random.** Every miss was the *generic*
   skill winning over the *specific* one: `dns` over `bind9-dns`, `tls` over
   `openssl-tls`, `archive` over `tar-archive`, `tracing` over `otel-tracing`.
   75% of misses had the gold inside the top-3 shortlist, so ranking works and
   the missing second stage is what converts it. The cookbook's two-stage shape
   (wide rank, then re-read the top three with their bodies) is the fix.

2. **Full descriptions hurt and cost 4.7x.** 60.6% against 68.2%, for
   $0.073 against $0.015. The short index is the right input, matching the
   cookbook's 60-character choice.

3. **The gate saturates.** Averaging three nouls compresses it: 56 of 66 cases
   landed in the 0.4-0.6 band, and in that band a skill exists 100% of the time.
   It almost never fires "no skill" (2 of 12 uncovered cases caught). Use one
   noul, or the minimum, or a lower threshold.

4. **Confidence orders outcomes but is inflated.** Correctness by confidence
   band: `<0.4` 0%, `0.4-0.6` 50%, `0.6-0.8` 37%, `0.8-1.0` 78%. Monotonic
   enough to route on, too optimistic to threshold absolutely. The right use is
   "escalate to stage 2 below 0.8", not "trust above 0.9".

5. **The memory gate is conservative**: precision 92%, recall 61%. All seven
   false negatives are the borderline cases; the one false positive is a
   duplicate — which the gate cannot see, because it never receives the existing
   memory index in the state. Feeding it is a design requirement, not a tweak.

6. **Effort does not discriminate.** 23 of 25 cases came back `high`. The
   rubric levels need rewriting, or effort should be a `score` over one axis.

## Known defects in this harness

- Skill calibration originally averaged the gate and the pick confidence into
  one curve; `analyze.py` now separates them.
- `workflow_cases.json` puts the gold at position 0 in 14 of 15 cases, so
  position bias is not controlled. The model picked position 0 only 7 of 15
  times, so it is judging, but the accuracy number is confounded.
- Gold labels are hand-written, not oracle. The five agent misses are all
  defensible synonyms (`code-reviewer` → `rust-engineer`), so 80% is a floor.
- One run, no repetitions: no variance estimate, no confidence interval.
- 66 skill cases is small, and the lookalikes were chosen deliberately hard.